// todo: ffi

pub struct RenderGraphNodeExecutionContext<'a> {
    // todo
    api: &'a mut (),
}

impl<'a> RenderGraphNodeExecutionContext<'a> {
    pub fn write_buffer(&mut self, buffer: RenderGraphBufferInput, data: &[u8]) {
        todo!();
    }

    pub fn draw(&mut self) {
        // todo: pass, pipeline, bind groups, vertex buffers, index buffer, indexed/non-indexed
        todo!();
    }

    pub fn submit(&mut self) {
        todo!();
    }
}

pub struct RenderGraphNode {
    // todo
    inputs_descriptors: (),
    // todo
    executor: (),
}

pub struct RenderGraphNodeBuilder<I> {
    identifier: String,
    desc: I,
}

impl RenderGraphNodeBuilder<()> {
    pub fn new(identifier: impl Into<String>) -> Self {
        Self {
            identifier: identifier.into(),
            desc: (),
        }
    }
}

impl<I: RenderGraphNodeInputDescriptor + RenderGraphTuple> RenderGraphNodeBuilder<I> {
    pub fn with<D: RenderGraphNodeInputDescriptor>(self, desc: D) -> RenderGraphNodeBuilder<I::With<D>> {
        RenderGraphNodeBuilder {
            identifier: self.identifier,
            desc: self.desc.into_tuple_with(desc),
        }
    }

    pub fn build(self, f: impl Fn(RenderGraphNodeExecutionContext, I::NodeInput)) -> RenderGraphNode {
        todo!()
    }
}

//

pub trait RenderGraphNodeInput {

}

pub trait RenderGraphNodeInputDescriptor {
    type NodeInput: RenderGraphNodeInput;
    
    fn into_input(self) -> Self::NodeInput;
}

//

pub struct RenderGraphBufferInput {

}

impl RenderGraphNodeInput for RenderGraphBufferInput {

}

pub struct RenderGraphBufferInputDescriptor {

}

impl RenderGraphNodeInputDescriptor for RenderGraphBufferInputDescriptor {
    type NodeInput = RenderGraphBufferInput;
    
    fn into_input(self) -> Self::NodeInput {
        todo!()
    }
}

//

pub trait RenderGraphTuple {
    type With<PN>;

    fn into_tuple_with<PN>(self, n: PN) -> Self::With<PN>;
}

macro_rules! impl_render_graph_tuple {
    ($($P: ident),*) => {
        impl<$($P),*> RenderGraphTuple for ($($P,)*) {
            type With<PN> = ($($P,)* PN,);
            
            fn into_tuple_with<PN>(self, n: PN) -> Self::With<PN> {
                #[allow(non_snake_case)]
                let (
                    $($P,)*
                ) = self;
                (
                    $($P,)*
                    n,
                )
            }
        }
    };
}

impl_render_graph_tuple!();
impl_render_graph_tuple!(P0);
impl_render_graph_tuple!(P0, P1);
impl_render_graph_tuple!(P0, P1, P2);
impl_render_graph_tuple!(P0, P1, P2, P3);
impl_render_graph_tuple!(P0, P1, P2, P3, P4);
impl_render_graph_tuple!(P0, P1, P2, P3, P4, P5);
impl_render_graph_tuple!(P0, P1, P2, P3, P4, P5, P6);
impl_render_graph_tuple!(P0, P1, P2, P3, P4, P5, P6, P7);

macro_rules! impl_render_graph_node_input_descriptor_tuple {
    ($($P: ident),*) => {
        impl<$($P: RenderGraphNodeInputDescriptor),*> RenderGraphNodeInputDescriptor for ($($P,)*) {
            type NodeInput = ($($P::NodeInput,)*);
            
            fn into_input(self) -> Self::NodeInput {
                #[allow(non_snake_case)]
                let (
                    $($P,)*
                ) = self;

                (
                    $($P.into_input(),)*
                )
            }
        }
    };
}

impl_render_graph_node_input_descriptor_tuple!();
impl_render_graph_node_input_descriptor_tuple!(P0);
impl_render_graph_node_input_descriptor_tuple!(P0, P1);
impl_render_graph_node_input_descriptor_tuple!(P0, P1, P2);
impl_render_graph_node_input_descriptor_tuple!(P0, P1, P2, P3);
impl_render_graph_node_input_descriptor_tuple!(P0, P1, P2, P3, P4);
impl_render_graph_node_input_descriptor_tuple!(P0, P1, P2, P3, P4, P5);
impl_render_graph_node_input_descriptor_tuple!(P0, P1, P2, P3, P4, P5, P6);
impl_render_graph_node_input_descriptor_tuple!(P0, P1, P2, P3, P4, P5, P6, P7);

macro_rules! impl_render_graph_node_input_tuple {
    ($($P: ident),*) => {
        impl<$($P: RenderGraphNodeInput),*> RenderGraphNodeInput for ($($P,)*) {
        }
    };
}

impl_render_graph_node_input_tuple!();
impl_render_graph_node_input_tuple!(P0);
impl_render_graph_node_input_tuple!(P0, P1);
impl_render_graph_node_input_tuple!(P0, P1, P2);
impl_render_graph_node_input_tuple!(P0, P1, P2, P3);
impl_render_graph_node_input_tuple!(P0, P1, P2, P3, P4);
impl_render_graph_node_input_tuple!(P0, P1, P2, P3, P4, P5);
impl_render_graph_node_input_tuple!(P0, P1, P2, P3, P4, P5, P6);
impl_render_graph_node_input_tuple!(P0, P1, P2, P3, P4, P5, P6, P7);

//

// todo
fn use_case() {
    let node = RenderGraphNodeBuilder::new("abc")
        .with(RenderGraphBufferInputDescriptor {
            // uniform/storage/storage-mut
            // ...
        })
        .build(|mut ctx, (buff_uniform,)| {
            ctx.write_buffer(buff_uniform, &[1, 2, 3]);

            ctx.draw()
        });

    // todo: schedule

    // todo: invoke
}