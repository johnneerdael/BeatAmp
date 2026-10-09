use anyhow::{Context, Result, ensure};
use rand_mt::Mt;
use std::{collections::BTreeMap, io::Cursor};

pub struct Texture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub size: [u32; 3],
}
pub struct Resources {
    pub textures: BTreeMap<String, Texture>,
    assets: BTreeMap<String, Vec<u8>>,
    pub substitutions: Vec<String>,
    samplers: Vec<wgpu::Sampler>,
}
impl Resources {
    pub fn new(device: &wgpu::Device, assets: BTreeMap<String, Vec<u8>>) -> Self {
        let samplers = (0..4)
            .map(|index| {
                device.create_sampler(&wgpu::SamplerDescriptor {
                    label: Some("declared-preset-sampling"),
                    address_mode_u: if index & 1 != 0 {
                        wgpu::AddressMode::Repeat
                    } else {
                        wgpu::AddressMode::ClampToEdge
                    },
                    address_mode_v: if index & 1 != 0 {
                        wgpu::AddressMode::Repeat
                    } else {
                        wgpu::AddressMode::ClampToEdge
                    },
                    address_mode_w: if index & 1 != 0 {
                        wgpu::AddressMode::Repeat
                    } else {
                        wgpu::AddressMode::ClampToEdge
                    },
                    mag_filter: if index & 2 != 0 {
                        wgpu::FilterMode::Linear
                    } else {
                        wgpu::FilterMode::Nearest
                    },
                    min_filter: if index & 2 != 0 {
                        wgpu::FilterMode::Linear
                    } else {
                        wgpu::FilterMode::Nearest
                    },
                    ..Default::default()
                })
            })
            .collect();
        Self {
            textures: BTreeMap::new(),
            assets,
            substitutions: vec![],
            samplers,
        }
    }
    pub fn sampler(&self, alias: &str, main_wrap: bool) -> &wgpu::Sampler {
        let (_, linear, wrap) = key(alias, main_wrap);
        &self.samplers[(if linear { 2 } else { 0 }) + if wrap { 1 } else { 0 }]
    }
    pub fn ensure(
        &mut self,
        alias: &str,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()> {
        let (name, _, _) = key(alias, true);
        ensure!(
            !["main_", "blur1_", "blur2_", "blur3_"]
                .iter()
                .any(|prefix| name.starts_with(prefix)),
            "malformed built-in texture alias: {alias}"
        );
        if matches!(name.as_str(), "main" | "blur1" | "blur2" | "blur3")
            || self.textures.contains_key(&name)
        {
            return Ok(());
        }
        let mut seed = 12345u32;
        for b in name.as_bytes() {
            seed = seed.wrapping_mul(33).wrapping_add(*b as u32);
        }
        let mut random = Mt::new(seed);
        let (width, height, depth, data) = if name.starts_with("noisevol_") {
            let n = 32u32;
            let data = (0..n * n * n * 4)
                .map(|_| (random.next_u32() >> 24) as u8)
                .collect();
            (n, n, n, data)
        } else if name.starts_with("noise_") {
            let n = 256u32;
            let cell = if name.contains("hq") {
                8
            } else if name.contains("mq") {
                4
            } else {
                1
            };
            let coarse_n = n / cell;
            let coarse: Vec<[u8; 4]> = (0..coarse_n * coarse_n)
                .map(|_| std::array::from_fn(|_| (random.next_u32() >> 24) as u8))
                .collect();
            let mut data = Vec::with_capacity((n * n * 4) as usize);
            for y in 0..n {
                for x in 0..n {
                    let gx = x as f32 / cell as f32;
                    let gy = y as f32 / cell as f32;
                    let ix = gx.floor() as u32;
                    let iy = gy.floor() as u32;
                    let fx = gx.fract();
                    let fy = gy.fract();
                    for channel in [0, 1, 2, 3] {
                        let at = |dx, dy| {
                            coarse[(((iy + dy) % coarse_n) * coarse_n + (ix + dx) % coarse_n)
                                as usize][channel] as f32
                        };
                        let value = (at(0, 0) * (1.0 - fx) + at(1, 0) * fx) * (1.0 - fy)
                            + (at(0, 1) * (1.0 - fx) + at(1, 1) * fx) * fy;
                        data.push(value as u8);
                    }
                }
            }
            (n, n, 1, data)
        } else {
            let path = if name.starts_with("rand") {
                let offset = name
                    .strip_prefix("rand")
                    .unwrap_or("0")
                    .parse::<usize>()
                    .unwrap_or(0);
                let index = (seed as usize + offset) % self.assets.len().max(1);
                self.assets
                    .keys()
                    .nth(index)
                    .cloned()
                    .context("random texture catalog missing")?
            } else {
                self.assets
                    .keys()
                    .find(|filename| {
                        std::path::Path::new(filename)
                            .file_stem()
                            .is_some_and(|stem| stem.to_string_lossy().eq_ignore_ascii_case(&name))
                    })
                    .cloned()
                    .unwrap_or_else(|| {
                        self.substitutions.push(format!(
                            "missing texture {name}: declared adaptation uses clouds2.jpg"
                        ));
                        "clouds2.jpg".into()
                    })
            };
            let bytes = self
                .assets
                .get(&path)
                .with_context(|| format!("missing adapted texture {path}"))?;
            let reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
            let (w, h) = reader.into_dimensions()?;
            ensure!(
                w > 0
                    && h > 0
                    && w <= 8192
                    && h <= 8192
                    && (w as u64 * h as u64 * 4) <= 32 * 1024 * 1024,
                "decoded texture budget exceeded"
            );
            let rgba = image::load_from_memory(bytes)?.into_rgba8();
            (w, h, 1, rgba.into_raw())
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&name),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: depth,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: if depth > 1 {
                wgpu::TextureDimension::D3
            } else {
                wgpu::TextureDimension::D2
            },
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: depth,
            },
        );
        let view = texture.create_view(&Default::default());
        self.textures.insert(
            name,
            Texture {
                texture,
                view,
                size: [width, height, depth],
            },
        );
        Ok(())
    }
    pub fn get(&self, alias: &str) -> Result<&Texture> {
        let (name, _, _) = key(alias, true);
        self.textures
            .get(&name)
            .with_context(|| format!("texture {name} was not prepared"))
    }
}
pub fn alias(name: &str) -> String {
    effect_model::shader::resource_alias(name)
}
pub fn key(alias: &str, main_wrap: bool) -> (String, bool, bool) {
    let mut name = alias
        .strip_prefix("sampler_")
        .or_else(|| alias.strip_prefix("texsize_"))
        .unwrap_or(alias);
    let mut linear = true;
    let mut wrap = name != "main" || main_wrap;
    if name.len() > 3
        && name.as_bytes()[2] == b'_'
        && matches!(&name[..2], "fc" | "fw" | "pc" | "pw")
    {
        linear = name.starts_with('f');
        wrap = name.as_bytes()[1] == b'w';
        name = &name[3..];
    }
    (name.to_owned(), linear, wrap)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compiler_names_preserve_resource_and_sampling_identity() {
        for name in [
            "sampler_fc_main_0",
            "sampler_fc_main_0_",
            "_sampler_fc_main_sampler_0_1_",
        ] {
            assert_eq!(key(&alias(name), true), ("main".into(), true, false));
        }
        assert_eq!(
            key(&alias("_sampler_pw_noise_lq_sampler_0_"), false),
            ("noise_lq".into(), false, true)
        );
        assert_eq!(key(&alias("sampler_rand00_0_"), true).0, "rand00");
        assert_eq!(
            key(&alias("sampler_noisevol_hq_0_"), true).0,
            key("texsize_noisevol_hq", true).0
        );
        assert_eq!(key(&alias("sampler_blur2_0_"), true).0, "blur2");
    }
}
