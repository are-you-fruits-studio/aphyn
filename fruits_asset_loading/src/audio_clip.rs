use std::path::Path;

use fruits_asset_storage::AssetStorageResource;
use fruits_audio::{AudioClip, AudioClipAssetMetadata, AudioStateResource};
use fruits_ecs::ResourcesHolderMut;
use fruits_serialization::*;

use crate::AssetLoader;

pub struct AudioClipHandleLoader<'a> {
    pub audio_state: &'a mut AudioStateResource,
    pub audio_clips: &'a mut AssetStorageResource<AudioClip>,
}

impl<'a> AudioClipHandleLoader<'a> {
    pub fn from_world(res: ResourcesHolderMut<'a>) -> Option<Self> {
        Some(unsafe { Self {
            audio_state: &mut *res.get_ptr::<AudioStateResource>()?,
            audio_clips: &mut *res.get_ptr::<AssetStorageResource<AudioClip>>()?,
        }})
    }
}
impl<'a, S: Copy + SerializerCtxState<AudioClipAssetMetadata>> AssetLoader<S> for AudioClipHandleLoader<'a> {
    type Asset = AudioClip;
    type SelfWithAnotherLifetime<'r> = AudioClipHandleLoader<'r>;

    fn create_loader<'r>(res: ResourcesHolderMut<'r>) -> Option<Self::SelfWithAnotherLifetime<'r>> {
        Self::SelfWithAnotherLifetime::from_world(res)
    }
    
    fn get_related_asset_storage(&mut self) -> &mut AssetStorageResource<Self::Asset> {
        self.audio_clips
    }
    
    fn load_from_serialized(&mut self, mut ctx: SerializerCtx<S>, value: &SerializedValue, assets_dir_path: impl AsRef<Path>) -> Option<Self::Asset> {
        AudioClipLoader {
            audio_state: self.audio_state,
        }.load_from_deserialized(ctx.deserialize_default("", value), assets_dir_path)
    }
    
}

pub struct AudioClipLoader<'a> {
    pub audio_state: &'a mut AudioStateResource,
}

impl<'a> AudioClipLoader<'a> {
    pub fn from_world(res: ResourcesHolderMut<'a>) -> Option<Self> {
        Some(Self {
            audio_state: res.into_get_mut::<AudioStateResource>()?,
        })
    }
    
    pub fn load_from_serialized(&mut self, value: &SerializedValue, assets_dir_path: impl AsRef<Path>) -> Option<AudioClip> {
        let SerializedValue::Composite(SerializedComposite { values: SerializedCompositeValues::Map(SerializedMap { values: value, .. }), .. }) = value else {
            return None;
        };

        let Some(SerializedValue::Primitive(SerializedPrimitive::String(raw_audio))) = value.get("raw_audio") else {
            return None;
        };

        let value = AudioClipAssetMetadata {
            raw_audio: raw_audio.clone(),
        };

        self.load_from_deserialized(value, assets_dir_path)
    }

    pub fn load_from_deserialized(&mut self, value: AudioClipAssetMetadata, assets_dir_path: impl AsRef<Path>) -> Option<AudioClip> {
        let mut path = assets_dir_path.as_ref().to_path_buf();
        path.push(value.raw_audio.as_str());

        self.load_from_wav_file(path, Some(value))
    }
    
    pub fn load_from_wav_file(&mut self, path: impl AsRef<Path>, meta: Option<AudioClipAssetMetadata>) -> Option<AudioClip> {
        let mut reader = match hound::WavReader::open(path) {
            Err(err) => {
                eprintln!("{}", err);
                return None;
            },
            Ok(r) => r,
        };
    
        let audio_spec = reader.spec();
    
        let channels = audio_spec.channels as usize;
        let sample_rate = audio_spec.sample_rate as usize;
    
        let max_value = (1 << (audio_spec.bits_per_sample - 1)) as f64;
    
        let raw_samples = match audio_spec.sample_format {
            hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<Vec<_>, _>>(),
            hound::SampleFormat::Int => reader.samples::<i32>().map(|s| s.map(|s| (s as f64 / max_value) as f32)).collect::<Result<Vec<_>, _>>()
        };
    
        let raw_samples = match raw_samples {
            Err(err) => {
                eprintln!("{}", err);
                return None;
            },
            Ok(s) => s,
        };
    
        let mut result = Vec::new();
    
        for chunk in raw_samples.chunks(channels) {
            let multi_sample = if channels == 1 {
                [chunk[0], chunk[0]]
            } else {
                [chunk[0], chunk[1]]
            };
        
            result.push(multi_sample[0]);
            result.push(multi_sample[1]);
        }
    
        if sample_rate != fruits_audio::AUDIO_SAMPLE_RATE {
            result = fruits_audio::resample_audio(&result, fruits_audio::AUDIO_CHANNELS_COUNT, sample_rate, fruits_audio::AUDIO_SAMPLE_RATE);
        }
    
        AudioClip::new(result.into(), self.audio_state, meta)
    }
}
