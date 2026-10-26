//! Draws the sky of the Weather tab on the GPU: one shader (`assets/weather/sky.metal`) paints every pixel from a
//! handful of numbers, so the clouds, the rain in its depths and the light of the sun come out as they do in life and
//! cost the CPU nothing. The same drawing can go to an image, which is how the checks look at it.

use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_foundation::NSString;
use objc2_metal::{
    MTLClearColor, MTLCommandBuffer, MTLCommandEncoder, MTLCommandQueue,
    MTLCreateSystemDefaultDevice, MTLDevice, MTLLibrary, MTLLoadAction, MTLOrigin, MTLPixelFormat,
    MTLPrimitiveType, MTLRegion, MTLRenderCommandEncoder, MTLRenderPassDescriptor,
    MTLRenderPipelineDescriptor, MTLRenderPipelineState, MTLSize, MTLStorageMode, MTLStoreAction,
    MTLTexture, MTLTextureDescriptor, MTLTextureUsage,
};
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};
use std::{
    ffi::c_void,
    ptr::NonNull,
    time::{Duration, Instant},
};

const SHADER: &str = include_str!("../assets/weather/sky.metal");
const FORMAT: MTLPixelFormat = MTLPixelFormat::BGRA8Unorm;
/// The most leaves the shader draws.
pub const MAX_LEAVES: usize = 32;

/// What the shader is told, as six groups of four numbers (see `U` in the shader).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Uniforms(pub [[f32; 4]; 6]);

pub struct Gpu {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    pipeline: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
}

impl Gpu {
    pub fn new() -> Result<Self, String> {
        let device = MTLCreateSystemDefaultDevice().ok_or("no GPU")?;
        let queue = device.newCommandQueue().ok_or("no command queue")?;
        let library = device
            .newLibraryWithSource_options_error(&NSString::from_str(SHADER), None)
            .map_err(|e| e.localizedDescription().to_string())?;
        let function = |name: &str| {
            library
                .newFunctionWithName(&NSString::from_str(name))
                .ok_or_else(|| format!("no shader function {name}"))
        };
        let descriptor = MTLRenderPipelineDescriptor::new();
        let (vertex, fragment) = (function("sky_vs")?, function("sky_fs")?);
        descriptor.setVertexFunction(Some(&*vertex));
        descriptor.setFragmentFunction(Some(&*fragment));
        // SAFETY: attachment 0 always exists.
        unsafe { descriptor.colorAttachments().objectAtIndexedSubscript(0) }.setPixelFormat(FORMAT);
        let pipeline = device
            .newRenderPipelineStateWithDescriptor_error(&descriptor)
            .map_err(|e| e.localizedDescription().to_string())?;
        Ok(Self {
            device,
            queue,
            pipeline,
        })
    }

    /// Makes a layer the sky can be drawn into.
    pub fn layer(&self) -> Retained<CAMetalLayer> {
        let layer = CAMetalLayer::new();
        layer.setDevice(Some(&self.device));
        layer.setPixelFormat(FORMAT);
        layer.setFramebufferOnly(true);
        layer.setOpaque(false);
        layer
    }

    fn encode(
        &self,
        target: &ProtocolObject<dyn MTLTexture>,
        uniforms: &Uniforms,
        leaves: &[[f32; 4]],
    ) -> Option<Retained<ProtocolObject<dyn MTLCommandBuffer>>> {
        let command = self.queue.commandBuffer()?;
        let pass = MTLRenderPassDescriptor::renderPassDescriptor();
        // SAFETY: attachment 0 always exists.
        let attachment = unsafe { pass.colorAttachments().objectAtIndexedSubscript(0) };
        attachment.setTexture(Some(target));
        attachment.setLoadAction(MTLLoadAction::Clear);
        attachment.setStoreAction(MTLStoreAction::Store);
        attachment.setClearColor(MTLClearColor {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 0.0,
        });
        let encoder = command.renderCommandEncoderWithDescriptor(&pass)?;
        let mut padded = [[0.0f32; 4]; MAX_LEAVES];
        for (slot, leaf) in padded.iter_mut().zip(leaves) {
            *slot = *leaf;
        }
        encoder.setRenderPipelineState(&self.pipeline);
        // SAFETY: the pointers are to live arrays of the sizes given, which the shader reads and does not keep.
        unsafe {
            encoder.setFragmentBytes_length_atIndex(
                NonNull::new(uniforms as *const Uniforms as *mut c_void)?,
                size_of::<Uniforms>(),
                0,
            );
            encoder.setFragmentBytes_length_atIndex(
                NonNull::new(padded.as_ptr() as *mut c_void)?,
                size_of_val(&padded),
                1,
            );
            encoder.drawPrimitives_vertexStart_vertexCount(MTLPrimitiveType::Triangle, 0, 3);
        }
        encoder.endEncoding();
        Some(command)
    }

    /// Draws a frame into the next drawable of the layer and shows it. False when the window server was slow to give a
    /// drawable back (it waits for one up to a second), which says the window is not being shown.
    pub fn draw(&self, layer: &CAMetalLayer, uniforms: &Uniforms, leaves: &[[f32; 4]]) -> bool {
        let asked = Instant::now();
        let Some(drawable) = layer.nextDrawable() else {
            return false;
        };
        let slow = asked.elapsed() > Duration::from_millis(100);
        if let Some(command) = self.encode(&drawable.texture(), uniforms, leaves) {
            command.presentDrawable(ProtocolObject::from_ref(&*drawable));
            command.commit();
        }
        !slow
    }

    /// Draws a frame into memory: `width` by `height` pixels of BGRA, top row first.
    pub fn render(
        &self,
        uniforms: &Uniforms,
        leaves: &[[f32; 4]],
        width: usize,
        height: usize,
    ) -> Option<Vec<u8>> {
        // SAFETY: a plain 2D texture of a size the device can make.
        let descriptor = unsafe {
            MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                FORMAT, width, height, false,
            )
        };
        descriptor.setUsage(MTLTextureUsage::RenderTarget);
        descriptor.setStorageMode(MTLStorageMode::Shared);
        let texture = self.device.newTextureWithDescriptor(&descriptor)?;
        let command = self.encode(&texture, uniforms, leaves)?;
        command.commit();
        command.waitUntilCompleted();
        let mut bytes = vec![0u8; width * height * 4];
        let region = MTLRegion {
            origin: MTLOrigin { x: 0, y: 0, z: 0 },
            size: MTLSize {
                width,
                height,
                depth: 1,
            },
        };
        // SAFETY: the buffer holds `height` rows of `width * 4` bytes, which the region reads.
        unsafe {
            texture.getBytes_bytesPerRow_fromRegion_mipmapLevel(
                NonNull::new(bytes.as_mut_ptr().cast::<c_void>())?,
                width * 4,
                region,
                0,
            );
        }
        Some(bytes)
    }
}
