use crate::graphics::{Antialiasing, Shell};
use crate::primitive;
use crate::quad;
use crate::text;
use crate::triangle;

use std::sync::{Arc, Mutex, RwLock};

#[derive(Clone)]
pub struct Engine {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) format: wgpu::TextureFormat,

    pub(crate) quad_pipeline: quad::Pipeline,
    pub(crate) text_pipeline: text::Pipeline,
    pub(crate) triangle_pipeline: triangle::Pipeline,
    #[cfg(any(feature = "image", feature = "svg"))]
    pub(crate) image_pipeline: crate::image::Pipeline,
    pub(crate) primitive_storage: Arc<RwLock<primitive::Storage>>,
    // Held while reconfiguring a surface to keep queue submissions from other
    // threads (currently the image loader worker) from racing with it. wgpu
    // versions up to 30.0.1 treat such a submission as a fatal error
    // (`ConfigureSurfaceError::GpuWaitTimeout`); see gfx-rs/wgpu#10296.
    pub(crate) submission_lock: Arc<Mutex<()>>,
    _shell: Shell,
}

impl Engine {
    pub fn new(
        _adapter: &wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        antialiasing: Option<Antialiasing>, // TODO: Initialize AA pipelines lazily
        shell: Shell,
    ) -> Self {
        Self {
            format,

            quad_pipeline: quad::Pipeline::new(&device, format),
            text_pipeline: text::Pipeline::new(&device, &queue, format),
            triangle_pipeline: triangle::Pipeline::new(&device, format, antialiasing),

            #[cfg(any(feature = "image", feature = "svg"))]
            image_pipeline: {
                let backend = _adapter.get_info().backend;

                crate::image::Pipeline::new(&device, format, backend)
            },

            primitive_storage: Arc::new(RwLock::new(primitive::Storage::default())),
            submission_lock: Arc::new(Mutex::new(())),

            device,
            queue,
            _shell: shell,
        }
    }

    #[cfg(any(feature = "image", feature = "svg"))]
    pub fn create_image_cache(&self) -> crate::image::Cache {
        self.image_pipeline.create_cache(
            &self.device,
            &self.queue,
            self.submission_lock.clone(),
            &self._shell,
        )
    }

    pub fn trim(&mut self) {
        self.text_pipeline.trim();

        self.primitive_storage
            .write()
            .expect("primitive storage should be writable")
            .trim();
    }
}
