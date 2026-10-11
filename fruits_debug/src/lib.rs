//! # fruits_debug
//!
//! Exposes a running world to an external debugging tool over a local TCP socket, so the tool can
//! request live information about the engine's state.
//!
//! # How to use
//!
//! #### Hosting the debug server
//!
//! Register the server module on the world builder. It binds a local socket and answers requests
//! from a connected debugging tool:
//!
//! ```no_run
//! use fruits_app::App;
//! use fruits_debug::add_module_to;
//!
//! let mut app = App::new();
//! add_module_to(app.ecs_mut());
//! app.run();
//! ```
//!
//! #### Sending a message to the connected peer
//!
//! A system can enqueue a message for the connected peer by pushing a `(type, payload)` pair onto
//! [`DebugConnectionResource::send_msg_queue`]. Message type `0` is reserved for the keep-alive
//! ping; [`msg_types::HIERARCHY`] requests the entity hierarchy:
//!
//! ```
//! use fruits_debug::{msg_types, DebugConnectionResource};
//! use fruits_ecs::ResMut;
//!
//! fn request_hierarchy(mut connection: ResMut<DebugConnectionResource>) {
//!     connection.send_msg_queue.push_back((msg_types::HIERARCHY, Vec::new()));
//! }
//! ```
//!
//! #### Reading received messages
//!
//! Messages received from the peer arrive on [`DebugConnectionResource::recv_msg_queue`] as
//! `(type, payload)` pairs. Drain it from a system and dispatch on the message type:
//!
//! ```
//! use fruits_debug::DebugConnectionResource;
//! use fruits_ecs::ResMut;
//!
//! fn drain_messages(mut connection: ResMut<DebugConnectionResource>) {
//!     while let Some((msg_type, payload)) = connection.recv_msg_queue.pop_back() {
//!         // dispatch on msg_type ...
//!         let _ = (msg_type, payload);
//!     }
//! }
//! ```
//!
//! #### Running as a connecting client
//!
//! [`add_module_as_client_to`] registers only the connection pump (receive, send, keep-alive),
//! without hosting a listener or generating responses. It is meant for a world that talks to a
//! debug server rather than hosting one:
//!
//! ```no_run
//! use fruits_app::App;
//! use fruits_debug::add_module_as_client_to;
//!
//! let mut app = App::new();
//! add_module_as_client_to(app.ecs_mut());
//! app.run();
//! ```
//!
//! # How to maintain
//!
//! #### Wire protocol
//!
//! Each message is a fixed 8-byte header followed by a payload. The header is two little-endian
//! `u32`s: the payload length, then the payload type. [`DebugMessageMetaData`] holds the parsed
//! header. [`debug_connection_send_system`] writes length, type, then payload bytes;
//! [`debug_connection_recv_system`] reads the header into `recv_active_msg_metadata`, then waits
//! until `recv_buffer` holds the whole payload before emitting a completed message onto
//! [`DebugConnectionResource::recv_msg_queue`]. Type `0` is the keep-alive ping;
//! [`msg_types::HIERARCHY`] is the only defined request.
//!
//! #### Connection lifecycle
//!
//! Only one connection is tracked at a time. In server mode [`host_debug_server`] lazily binds a
//! [`std::net::TcpListener`] on `127.0.0.1:55643`, sets it non-blocking, and accepts a single
//! stream into [`DebugConnectionResource::active_stream`]. While no stream is connected it calls
//! [`DebugConnectionResource::reset`] every frame, so anything queued in the meantime is
//! dropped. All socket IO is non-blocking: a read error other than `WouldBlock` / `TimedOut`,
//! or any write error, calls [`DebugConnectionResource::reset`] to drop the stream and clear
//! the buffers and queues. Each frame the receive system reads at most 1024 bytes and
//! completes at most one message, and the send system writes at most one queued message.
//! [`debug_connection_ping_system`] enqueues an empty type-`0` message when no message has
//! been sent or received yet, or at least one second has passed since the last one, so an
//! idle connection still produces traffic.
//!
//! #### Responses
//!
//! [`generate_response_system`] pops one message per frame from the back of
//! [`DebugConnectionResource::recv_msg_queue`]. For a [`msg_types::HIERARCHY`] request it
//! queries every [`EntityId`] and replies with one pair of little-endian
//! `u32`s per entity — the entity's version-index `index` then `version`. Other message types
//! are dropped.
//!
//! #### System ordering
//!
//! All systems run in [`fruits_ecs::Schedule::Update`] under the [`SYSTEM_GROUP`] group. The
//! server orders them so a connection is established before it is used and a response is produced
//! before it is sent: [`host_debug_server`] runs before receive, send, and ping;
//! [`debug_connection_recv_system`] runs before [`generate_response_system`], which runs before
//! [`debug_connection_send_system`]. The client variant registers only receive, send, and ping
//! and relies on `active_stream` being supplied elsewhere.
//!
//! #### Caveats
//!
//! The resources are not yet exposed across the FFI boundary (see the `// todo: support ffi`
//! notes), and binding, switching sockets to non-blocking mode, and flushing use `unwrap`, so
//! those socket failures panic rather than surface as errors. The crate's `fruits_debug` binary target (`src/main.rs`) is a manual harness
//! that hosts the server over a world seeded with 100 entities.

use std::{
    collections::VecDeque,
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream},
    time::Instant,
};

use fruits_ecs::{EntityId, ResMut, Resource, Schedule, WorldBuilder, WorldDataMut};

pub const SYSTEM_GROUP: &str = "fruits_debug";

pub fn add_module_to(world: &mut WorldBuilder) {
    world
        .data_mut()
        .resources_mut()
        .insert(DebugServerResource::default());
    world
        .data_mut()
        .resources_mut()
        .insert(DebugConnectionResource::default());

    world
        .behavior_mut()
        .get_mut(Schedule::Update)
        .group(SYSTEM_GROUP)
        .insert_child_system(host_debug_server)
        .insert_child_system(debug_connection_recv_system)
        .insert_child_system(debug_connection_send_system)
        .insert_child_system(generate_response_system)
        .insert_child_system(debug_connection_ping_system);

    world
        .behavior_mut()
        .get_mut(Schedule::Update)
        .order_system(host_debug_server)
        .before_system(debug_connection_recv_system);
    world
        .behavior_mut()
        .get_mut(Schedule::Update)
        .order_system(host_debug_server)
        .before_system(debug_connection_send_system);
    world
        .behavior_mut()
        .get_mut(Schedule::Update)
        .order_system(host_debug_server)
        .before_system(debug_connection_ping_system);
    world
        .behavior_mut()
        .get_mut(Schedule::Update)
        .order_system(debug_connection_recv_system)
        .before_system(generate_response_system);
    world
        .behavior_mut()
        .get_mut(Schedule::Update)
        .order_system(generate_response_system)
        .before_system(debug_connection_send_system);
}

pub fn add_module_as_client_to(world: &mut WorldBuilder) {
    world
        .data_mut()
        .resources_mut()
        .insert(DebugServerResource::default());
    world
        .data_mut()
        .resources_mut()
        .insert(DebugConnectionResource::default());

    world
        .behavior_mut()
        .get_mut(Schedule::Update)
        .group(SYSTEM_GROUP)
        .insert_child_system(debug_connection_recv_system)
        .insert_child_system(debug_connection_send_system)
        .insert_child_system(debug_connection_ping_system);
}

pub mod msg_types {
    pub const HIERARCHY: u32 = 1;
}

pub fn generate_response_system(world: WorldDataMut) {
    let (res, ec, _evt) = world.into_tuple_mut();

    let connection_res = res.into_get_mut::<DebugConnectionResource>().unwrap();

    let Some(msg) = connection_res.recv_msg_queue.pop_back() else {
        return;
    };

    if msg.0 == msg_types::HIERARCHY {
        let mut response = Vec::new();
        for ent in ec.query::<EntityId>().iter() {
            let vi = ent.version_index();

            response.extend_from_slice(&(vi.index as u32).to_le_bytes());
            response.extend_from_slice(&(vi.version as u32).to_le_bytes());
        }
        connection_res.send_msg_queue.push_back((msg_types::HIERARCHY, response));
    }
}

pub fn host_debug_server(mut server_res: ResMut<DebugServerResource>, mut connection_res: ResMut<DebugConnectionResource>) {
    if server_res.listener.is_none() {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 55643)).unwrap();
        server_res.listener = Some(listener);
    }

    if connection_res.active_stream.is_none() {
        connection_res.reset();

        let listener = server_res.listener.as_ref().unwrap();
        listener.set_nonblocking(true).unwrap();
        let Ok(connection) = listener.accept() else {
            return;
        };

        connection.0.set_nonblocking(true).unwrap();
        connection_res.active_stream = Some(connection.0);
        println!("connected");
    }
}

pub fn debug_connection_ping_system(mut connection_res: ResMut<DebugConnectionResource>) {
    if connection_res.active_stream.is_none() {
        return;
    };

    let should_ping = match connection_res.last_msg_time {
        Some(last_msg_time) => last_msg_time.elapsed().as_secs_f32() >= 1.0,
        None => true,
    };

    if should_ping {
        connection_res.send_msg_queue.push_back((0, Vec::new()));
    }
}

pub fn debug_connection_recv_system(mut connection_res: ResMut<DebugConnectionResource>) {
    let Some(stream) = &mut connection_res.active_stream else {
        return;
    };

    let mut buf = [0_u8; 1024];

    let bytes_read = match stream.read(&mut buf) {
        Ok(bytes_read) => bytes_read,
        Err(err) => {
            match err.kind() {
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => (),
                _ => {
                    connection_res.reset();
                    println!("disconnected");
                }
            }

            return;
        }
    };

    if bytes_read == 0 {
        return;
    }

    connection_res.last_msg_time = Some(Instant::now());

    connection_res.recv_buffer.extend(buf[..bytes_read].into_iter());

    if connection_res.recv_active_msg_metadata.is_none() {
        if connection_res.recv_buffer.len() < 8 {
            return;
        }

        let mut payload_size_bytes = [0_u8; 4];
        for b in payload_size_bytes.iter_mut() {
            *b = connection_res.recv_buffer.pop_front().unwrap();
        }

        let payload_size = u32::from_le_bytes(payload_size_bytes);

        let mut payload_type_bytes = [0_u8; 4];
        for b in payload_type_bytes.iter_mut() {
            *b = connection_res.recv_buffer.pop_front().unwrap();
        }

        let payload_type = u32::from_le_bytes(payload_type_bytes);

        connection_res.recv_active_msg_metadata = Some(DebugMessageMetaData {
            payload_size,
            payload_type,
        });
    }

    let metadata = connection_res.recv_active_msg_metadata.unwrap();

    if connection_res.recv_buffer.len() < metadata.payload_size as usize {
        return;
    }

    let mut msg_buffer = Vec::with_capacity(metadata.payload_size as usize);

    for _ in 0..metadata.payload_size {
        msg_buffer.push(connection_res.recv_buffer.pop_front().unwrap());
    }

    connection_res.recv_active_msg_metadata = None;

    println!(
        "msg received ({}, {}): {:?}",
        metadata.payload_size, metadata.payload_type, msg_buffer
    );
    connection_res.recv_msg_queue.push_back((metadata.payload_type, msg_buffer));
}

pub fn debug_connection_send_system(mut connection_res: ResMut<DebugConnectionResource>) {
    if connection_res.active_stream.is_none() {
        return;
    }

    let Some(msg) = connection_res.send_msg_queue.pop_front() else {
        return;
    };

    let stream = connection_res.active_stream.as_mut().unwrap();

    if let Err(err) = stream.write_all(&(msg.1.len() as u32).to_le_bytes()) {
        eprintln!("{}", err);
        connection_res.reset();
        println!("disconnected");
        return;
    }
    if let Err(err) = stream.write_all(&(msg.0).to_le_bytes()) {
        eprintln!("{}", err);
        connection_res.reset();
        println!("disconnected");
        return;
    }
    if let Err(err) = stream.write_all(&msg.1) {
        eprintln!("{}", err);
        connection_res.reset();
        println!("disconnected");
        return;
    }

    stream.flush().unwrap();

    connection_res.last_msg_time = Some(Instant::now());
}

// todo: support ffi
#[derive(Resource, Default)]
pub struct DebugServerResource {
    listener: Option<TcpListener>,
}

// todo: support ffi
#[derive(Resource, Default)]
pub struct DebugConnectionResource {
    pub active_stream: Option<TcpStream>,
    pub send_msg_queue: VecDeque<(u32, Vec<u8>)>,
    pub recv_msg_queue: VecDeque<(u32, Vec<u8>)>,
    recv_buffer: VecDeque<u8>,
    recv_active_msg_metadata: Option<DebugMessageMetaData>,
    pub last_msg_time: Option<Instant>,
}

impl DebugConnectionResource {
    pub fn reset(&mut self) {
        self.active_stream = None;
        self.send_msg_queue.clear();
        self.recv_msg_queue.clear();
        self.recv_buffer.clear();
        self.recv_active_msg_metadata = None;
        self.last_msg_time = None;
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct DebugMessageMetaData {
    pub payload_size: u32,
    pub payload_type: u32,
}
