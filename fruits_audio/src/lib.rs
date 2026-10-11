//! # fruits_audio
//!
//! Audio playback for the Fruits engine: it turns sound clips attached to world entities into
//! mixed output on the machine's speakers.
//!
//! # How to use
//!
//! #### Enabling audio in a world
//!
//! Audio is part of the engine's default modules (`add_defult_modules_to`). To enable it on its
//! own, call [`add_audio_module_to`] with the world builder:
//!
//! ```ignore
//! use fruits_engine::*;
//!
//! let mut app = App::new();
//! add_audio_module_to(app.ecs_mut().as_mut());
//! ```
//!
//! #### Playing a sound
//!
//! Attach an [`AudioSource`] referencing an [`AudioClip`] to an entity. Clips are loaded from
//! `.asset` files by the asset layer (`fruits_asset_loading`) at startup and looked up by their
//! asset key in the `AssetStorageResource<AudioClip>`:
//!
//! ```ignore
//! use fruits_engine::*;
//!
//! fn start_music(mut world: WorldDataMut) {
//!     let (res, mut ent, _evt) = world.as_tuple_mut();
//!
//!     let clips = res.get::<AssetStorageResource<AudioClip>>().unwrap();
//!     let clip = clips.get_registered("Through space.asset").unwrap().clone();
//!
//!     let entity = ent.create_entity();
//!     ent.add_component(entity, AudioSource {
//!         clip,
//!         playback_time: 0.0,
//!         volume: 1.0,
//!         should_force_playback_time: true,
//!         is_playing: true,
//!         is_looped: true,
//!     }).ok().unwrap();
//! }
//! ```
//!
//! The [`AudioSource`] fields drive playback every [`Schedule::Update`]: flip `is_playing` to
//! start or pause, set `is_looped` to repeat, scale the output with `volume`, and write
//! `playback_time` (in seconds) together with `should_force_playback_time = true` to seek.
//! Once a non-looped clip reaches its end the system clears `is_playing`. A source whose clip
//! handle does not resolve is stopped and its `playback_time` reset to `0.0`.
//!
//! #### Reading the played samples
//!
//! [`AudioStateResource::last_samples`] returns the most recently played samples as an interleaved
//! stereo buffer (`L, R, L, R, ...` at 48&nbsp;kHz) — useful for waveform visualizations:
//!
//! ```ignore
//! fn visualize(audio: Res<AudioStateResource>) {
//!     let samples = audio.last_samples(); // interleaved L, R, L, R, ... at 48 kHz
//!     // ...
//! }
//! ```
//!
//! #### Resampling raw audio to the engine rate
//!
//! A clip recorded at some other sample rate has to be brought to the engine's 48&nbsp;kHz before it
//! can be played. [`resample_audio`] converts an interleaved buffer from one sample rate to
//! another, so callers preparing their own clips can bring them to the engine's rate:
//!
//! ```
//! use fruits_audio::{resample_audio, AUDIO_CHANNELS_COUNT, AUDIO_SAMPLE_RATE};
//!
//! // one second of silent stereo audio captured at 44.1 kHz
//! let at_44100 = vec![0.0f32; 44_100 * AUDIO_CHANNELS_COUNT];
//!
//! let at_48000 = resample_audio(&at_44100, AUDIO_CHANNELS_COUNT, 44_100, AUDIO_SAMPLE_RATE);
//! assert_eq!(at_48000.len(), AUDIO_SAMPLE_RATE * AUDIO_CHANNELS_COUNT);
//! ```
//!
//! # How to maintain
//!
//! #### Constants and formats
//!
//! Everything is fixed to interleaved stereo float samples at 48&nbsp;kHz: [`AUDIO_SAMPLE_RATE`] and
//! [`AUDIO_CHANNELS_COUNT`] encode those assumptions, and an [`AudioClip`] stores its samples in
//! exactly that layout inside an [`FfiVec`], together with its optional
//! [`AudioClipAssetMetadata`] (the serializable asset-file settings, kept so the clip can be
//! saved back). [`AudioClip::new`] returns `None` for buffers whose length is not a multiple of
//! [`AUDIO_CHANNELS_COUNT`] and stamps each clip with a unique `id` drawn from the
//! [`AudioStateResource`]'s `next_audio_clip_id` counter (starting at `1`; `0` marks the empty
//! placeholder clip of a new playback); the `id` is how the update system detects that a
//! source's clip changed and needs re-copying.
//!
//! #### Shared state across two threads
//!
//! The mixing happens on [`cpal`]'s audio callback thread, while gameplay touches audio from the
//! ECS thread. Both reach the same `AudioState` — the set of `AudioActivePlayback`s and the
//! last mixed buffer — through an `Arc<Mutex<AudioState>>`. That handle is wrapped twice for FFI:
//! [`WrappedAudioStateHandle`] holds it inside an [`FfiDroppable`], and the live [`cpal::Stream`]
//! is likewise stored in [`AudioStateResource`]'s `_stream` field so dropping the resource stops
//! the stream. Because the raw pointers in those wrappers are not auto-`Send`/`Sync`,
//! [`AudioStateResource`] implements both traits by hand; the contract that keeps that sound is the
//! `Mutex`, which serializes every access to `AudioState`.
//!
//! #### Opening the stream
//!
//! `start_playback` takes the default host's default output device and builds an output stream
//! with a hard-coded [`StreamConfig`] of 2 channels at 48&nbsp;kHz and the default buffer size. The
//! mixing closure runs per callback: it zeroes the output buffer, then for every playing
//! `AudioActivePlayback` walks `sample_index` forward one multisample at a time. When the index
//! runs past the clip it wraps (looping) or stops the playback. Each sample is scaled by the
//! playback's `volume` and added to the output; the closure averages the stereo pair for a mono
//! output and copies the channels through otherwise, although the hard-coded config always
//! requests stereo. After mixing it stores the
//! buffer into `last_played_samples`. The closure currently clones each clip's samples into its
//! playback (see the `// todo:` about reusing the asset buffer via an FFI-capable `Arc`).
//!
//! #### The update system
//!
//! [`add_audio_module_to`] opens the stream, inserts the [`AudioStateResource`] and an empty
//! `AssetStorageResource<AudioClip>`, and schedules `audio_system` into the
//! [`SYSTEM_GROUP_AUDIO`] group on [`Schedule::Update`]. Each tick it locks the `AudioState`
//! and: copies `last_played_samples` out into the resource's `last_samples` (the public
//! mirror), drops playbacks whose entity no longer matches a queried [`AudioSource`], then
//! creates or syncs a playback for each live source — pushing `is_playing`/`is_looped`/`volume`
//! down, replacing the playback's clip when the `id` differs, and
//! either forcing `sample_index` from `playback_time` (on `should_force_playback_time`) or writing
//! `playback_time` back from `sample_index`. The position read back into `playback_time` is clamped
//! to `0.0..=1.0` seconds, so a source playing a clip longer than one second reports a saturated
//! time — a known limitation to be aware of before relying on `playback_time` for long clips.
//!
//! #### Resampling internals
//!
//! [`resample_audio`] resamples per channel by sampling the source at the new rate's fractional
//! positions and feeding the four surrounding samples through `interpolate_cubic`, a Catmull-Rom
//! style cubic. `interpolate_cubic` carries a `// todo:` marking it for a future move into a shared
//! math crate. The longer-term scope of the crate — more input formats, native sample rates and bit
//! depths, and 3D spatialization — is tracked in the `// todo:` list at the top of the file.

use std::{collections::HashMap, sync::{Arc, Mutex}};

use cpal::{OutputCallbackInfo, StreamConfig, traits::{DeviceTrait, HostTrait, StreamTrait}};
use fruits_asset_storage::*;
use fruits_ecs::*;
use fruits_ffi::{FfiDroppable, FfiOption, FfiString, FfiVec};
use fruits_serialization::*;

// todo:
// + read different bits_per_sample (and sample type - f32, i32, i16, i8) -> convert to f32
// + read different channels_count -> convert to 2 channels
// + read different sample_rate -> convert to 48kHz (use cubic interpolation)
// + playback different channels_count (1 or 2) -> average channels if needed
// + create convenient ecs API for playback
// + ffi
// + create convenient assets API
// - read different audio formats (wav, mp3, ogg, etc.)
// - playback different sample_rates -> resample to native
// - playback different bits_per_sample (and sample type - f32, i32, i16, i8) -> convert from f32
// - make audio spatial (3D) in virtual world

pub const AUDIO_SAMPLE_RATE: usize = 48000;
pub const AUDIO_CHANNELS_COUNT: usize = 2;

pub const SYSTEM_GROUP_AUDIO: &'static str = "fruits_audio";

pub fn add_audio_module_to(mut world: WorldBuilderMut) {
    let audio_state = Arc::new(Mutex::new(AudioState::new()));
    let stream = start_playback(Arc::clone(&audio_state));

    world
        .data_mut()
        .resources_mut()
        .insert(AudioStateResource::new(FfiDroppable::new(stream), WrappedAudioStateHandle::new(Arc::clone(&audio_state))));

    world
        .data_mut()
        .resources_mut()
        .insert(AssetStorageResource::<AudioClip>::new());

    world.behavior_mut()
        .get_mut(Schedule::Update)
        .group(SYSTEM_GROUP_AUDIO)
        .insert_child_system(audio_system);
}


#[repr(C)]
#[derive(Serializable, Clone, Default)]
pub struct AudioClipAssetMetadata {
    pub raw_audio: FfiString,
}

#[repr(C)]
#[derive(Clone)]
pub struct AudioClip {
    // interleaved stereo float [-1.0; 1.0] 48kHz samples
    samples: FfiVec<f32>,
    id: u64,
    meta: FfiOption<AudioClipAssetMetadata>,
}

impl AudioClip {
    pub fn new(samples: FfiVec<f32>, audio_state: &mut AudioStateResource, meta: Option<AudioClipAssetMetadata>) -> Option<Self> {
        if samples.len() % AUDIO_CHANNELS_COUNT as u64 != 0 {
            return None;
        }

        let id = {
            let id = audio_state.next_audio_clip_id;
            audio_state.next_audio_clip_id += 1;
            id
        };

        Some(Self {
            samples,
            id,
            meta: meta.into(),
        })
    }

    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn meta(&self) -> Option<&AudioClipAssetMetadata> {
        self.meta.as_ref()
    }
}

#[repr(C)]
#[derive(Component)]
pub struct AudioSource {
    pub clip: AssetHandle<AudioClip>,
    pub playback_time: f64,
    pub volume: f32,
    pub should_force_playback_time: bool,
    pub is_playing: bool,
    pub is_looped: bool,
}

#[repr(C)]
#[derive(Resource)]
pub struct AudioStateResource {
    _stream: FfiDroppable,
    state: WrappedAudioStateHandle,
    last_samples: FfiVec<f32>,
    next_audio_clip_id: u64,
}

impl AudioStateResource {
    pub fn new(stream: FfiDroppable, state: WrappedAudioStateHandle) -> Self {
        Self {
            _stream: stream,
            state,
            last_samples: FfiVec::new(),
            next_audio_clip_id: 1,
        }
    }
}

unsafe impl Send for AudioStateResource {}
unsafe impl Sync for AudioStateResource {}

impl AudioStateResource {
    pub fn last_samples(&self) -> &[f32] {
        &self.last_samples
    }
}

#[repr(C)]
pub struct WrappedAudioStateHandle {
    state: FfiDroppable,
}

impl WrappedAudioStateHandle {
    pub fn new(state: Arc<Mutex<AudioState>>) -> Self {
        Self {
            state: FfiDroppable::new(state),
        }
    }

    unsafe fn as_raw(&self) -> &Arc<Mutex<AudioState>> {
        unsafe {
            &*(self.state.get() as *mut Arc<Mutex<AudioState>>)
        }
    }
}

#[repr(C)]
struct AudioActivePlayback {
    clip: AudioClip,
    sample_index: usize,
    is_playing: bool,
    is_looped: bool,
    volume: f32,
}

pub struct AudioState {
    active_playbacks: HashMap<EntityId, AudioActivePlayback>,
    last_played_samples: Vec<f32>,
}

impl AudioState {
    pub fn new() -> Self {
        Self {
            active_playbacks: HashMap::new(),
            last_played_samples: Vec::new(),
        }
    }
}

fn audio_system(
    mut state: ResMut<AudioStateResource>,
    mut source_q: WorldQuery<(EntityId, &mut AudioSource)>,
    clips: Res<AssetStorageResource<AudioClip>>,
) {
    let mut last_samples = std::mem::replace(&mut state.last_samples, FfiVec::new());

    {
        let state = unsafe { state.state.as_raw() };
        let audio_state = &mut *state.lock().unwrap();
       
        last_samples.resize(audio_state.last_played_samples.len() as u64, 0.0);
        last_samples.copy_from_slice(&audio_state.last_played_samples);

        // remove redundant playbacks
        let mut playbacks_to_remove = Vec::new();
        for (entity, _) in &audio_state.active_playbacks {
            if source_q.get(*entity).is_none() {
                playbacks_to_remove.push(*entity);
            }
        }
        for playback_to_remove in playbacks_to_remove {
            audio_state.active_playbacks.remove(&playback_to_remove);
        }

        // add missing playbacks
        // sync all playbacks and components
        for (entity, source) in source_q.iter_mut() {
            let clip = clips.get(&source.clip);

            let playback = audio_state.active_playbacks
                .entry(entity)
                .or_insert_with(|| AudioActivePlayback {
                    clip: AudioClip {
                        samples: FfiVec::new(),
                        id: 0,
                        meta: None.into(),
                    },
                    sample_index: 0,
                    is_playing: false,
                    is_looped: false,
                    volume: 1.0,
                });

            let Some(clip) = clip else {
                source.should_force_playback_time = false;
                source.is_playing = false;
                source.playback_time = 0.0;
                continue;
            };

            if clip.id != playback.clip.id {
                playback.clip = clip.clone();
            }

            playback.is_playing = source.is_playing;
            playback.is_looped = source.is_looped;
            playback.volume = source.volume;

            if source.should_force_playback_time {
                playback.sample_index = ((source.playback_time * AUDIO_SAMPLE_RATE as f64) as usize).clamp(0, playback.clip.samples.len() as usize - 1);
                source.should_force_playback_time = false;
            } else {
                source.playback_time = (playback.sample_index as f64 / AUDIO_SAMPLE_RATE as f64).clamp(0.0, 1.0);
            }

            if playback.sample_index >= playback.clip.samples.len() as usize / AUDIO_CHANNELS_COUNT {
                source.is_playing = false;
            }
        }
    }

    _ = std::mem::replace(&mut state.last_samples, last_samples);
}

fn start_playback(state: Arc<Mutex<AudioState>>) -> cpal::Stream {
    let host = cpal::default_host();

    // let input_device = host.default_input_device().unwrap();
    let output_device = host.default_output_device().unwrap();

    let output_config = StreamConfig {
        buffer_size: cpal::BufferSize::Default,
        channels: 2,
        sample_rate: 48000,
    };

    println!("Audio output config: channels: {}, sample_rate: {}, buffer_size: {:?}", output_config.channels, output_config.sample_rate, output_config.buffer_size);
    let channels_count = output_config.channels as usize;

    let stream = output_device.build_output_stream(
        &output_config,
        move |output_samples: &mut [f32], _: &OutputCallbackInfo| {
            for sample in output_samples.iter_mut() {
                *sample = 0.0;
            }
            let output_miltisamples_count = output_samples.len() / channels_count;

            {
                let state = &mut *state.lock().unwrap();

                for (_, playback) in &mut state.active_playbacks {
                    if !playback.is_playing {
                        continue;
                    }

                    // todo: reuse the same buffer from audio asset, not clone it (probably needs FfiArc to support ffi)
                    let playback_multisamples_count = playback.clip.samples.len() as usize / AUDIO_CHANNELS_COUNT;

                    for s in 0..output_miltisamples_count {
                        let handle_over_sample = |playback: &mut AudioActivePlayback| {
                            if playback.sample_index >= playback_multisamples_count {
                                if playback.is_looped {
                                    playback.sample_index %= playback_multisamples_count;
                                } else {
                                    playback.sample_index = playback_multisamples_count;
                                    playback.is_playing = false;
                                }
                            }
                        };

                        handle_over_sample(playback);

                        if playback.sample_index >= playback_multisamples_count {
                            break;
                        }

                        let playback_multisample = &(&*playback.clip.samples)[((playback.sample_index) * AUDIO_CHANNELS_COUNT)..((playback.sample_index + 1) * AUDIO_CHANNELS_COUNT)];
                        if channels_count == 1 {
                            output_samples[s] += playback_multisample.iter().sum::<f32>() * playback.volume / playback_multisample.len() as f32;
                        } else {
                            for c in 0..channels_count.min(AUDIO_CHANNELS_COUNT) {
                                output_samples[s * channels_count + c] += playback_multisample[c] * playback.volume;
                            }
                        }

                        playback.sample_index += 1;

                        handle_over_sample(playback);
                    }
                }

                state.last_played_samples.resize(output_samples.len(), 0.0);
                state.last_played_samples.copy_from_slice(output_samples);
            }
        },
        move |err| {
            eprintln!("Audio stream error: {}", err);
        },
        None,
    ).unwrap();

    stream.play().unwrap();

    return stream;
}

pub fn resample_audio(samples: &[f32], channels: usize, old_sample_rate: usize, new_sample_rate: usize) -> Vec<f32> {
    let original_multisamples_count = samples.len() / channels;
    let new_multisamples_count = original_multisamples_count * new_sample_rate / old_sample_rate;
    let mut resampled_samples = Vec::with_capacity(new_sample_rate);

    for i in 0..new_multisamples_count {
        let progress = i as f64 / (new_multisamples_count - 1) as f64;

        let scaled_original_progress = progress * (original_multisamples_count - 1) as f64;
        let j0 = (scaled_original_progress.floor() as usize).clamp(0, original_multisamples_count - 1);
        let jt = (scaled_original_progress - j0 as f64) as f32;
        let j1 = (j0 + 1).clamp(0, original_multisamples_count - 1);
        let j2 = (j1 + 1).clamp(0, original_multisamples_count - 1);
        let jn1 = (j0.max(1) - 1).clamp(0, original_multisamples_count - 1);

        for c in 0..channels {
            let resampled_sample = interpolate_cubic(
                samples[jn1 * channels + c],
                samples[j0 * channels + c],
                samples[j1 * channels + c],
                samples[j2 * channels + c],
                jt,
            );

            resampled_samples.push(resampled_sample);
        }
    }

    resampled_samples
}

// todo: to math crate
fn interpolate_cubic(pn1: f32, p0: f32, p1: f32, p2: f32, t: f32) -> f32 {
    let a = -pn1 * 0.5 + p0 * 1.5 - p1 * 1.5 + p2 * 0.5;
    let b = pn1 - p0 * 2.5 + p1 * 2.0 - p2 * 0.5;
    let c = -pn1 * 0.5 + p1 * 0.5;
    let d = p0;

    let tt = t * t;
    let ttt = tt * t;

    a * ttt + b * tt + c * t + d
}