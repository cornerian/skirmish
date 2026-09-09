//! Thread-confined SDL window ownership for wgpu's native surface boundary.
//!
//! wgpu's safe surface constructor requires a `Send + Sync` window provider.
//! SDL windows and their video subsystem must stay on the SDL thread. Keep the
//! raw-handle operation here instead of asserting those traits on an SDL window.
//! This module is private: the public renderer must not return its surface or
//! acquired surface textures, which could otherwise outlive the owning window.

use std::{marker::PhantomData, rc::Rc};

use anyhow::{Context, Result, ensure};
use sdl3::video::Window;
use wgpu::rwh::{HasDisplayHandle, HasWindowHandle};

/// Keeps the surface, window, and SDL video subsystem on their creating thread.
pub(crate) struct SdlSurface {
    // Fields drop in declaration order. The native window and its SDL video
    // subsystem must remain alive while wgpu destroys the surface and instance.
    surface: wgpu::Surface<'static>,
    instance: wgpu::Instance,
    window: Window,
    _main_thread: PhantomData<Rc<()>>,
}

impl SdlSurface {
    pub(crate) fn new(window: Window) -> Result<Self> {
        Self::validate_driver(&window)?;
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        // wgpu 30 needs an owned Send + Sync display provider for presenting
        // through GLES, especially on Wayland. SDL cannot supply that safely.
        // Vulkan, Metal, and DX12 consume the surface handles directly instead.
        descriptor.backends &=
            wgpu::Backends::VULKAN | wgpu::Backends::METAL | wgpu::Backends::DX12;
        ensure!(
            !descriptor.backends.is_empty(),
            "SDL window presentation requires Vulkan, Metal, or DX12; \
             the selected WGPU_BACKEND only supports other backends \
             (GLES remains available for headless rendering)"
        );
        Self::from_parts(wgpu::Instance::new(descriptor), window)
    }

    fn validate_driver(window: &Window) -> Result<()> {
        // SDL's raw-window-handle implementation panics for unsupported Unix
        // video drivers, including "dummy". Report that as a startup error.
        #[cfg(all(
            unix,
            not(target_os = "macos"),
            not(target_os = "ios"),
            not(target_os = "android")
        ))]
        ensure!(
            matches!(window.subsystem().current_video_driver(), "x11" | "wayland"),
            "SDL video driver {:?} cannot provide a graphics surface; use X11 or Wayland",
            window.subsystem().current_video_driver()
        );
        // Other native platforms have a fixed SDL raw-handle implementation.
        let _ = window;
        Ok(())
    }

    #[allow(unsafe_code)]
    fn from_parts(instance: wgpu::Instance, window: Window) -> Result<Self> {
        Self::validate_driver(&window)?;

        let raw_window_handle = window
            .window_handle()
            .context("reading SDL window handle")?
            .as_raw();
        let raw_display_handle = window
            .display_handle()
            .context("reading SDL display handle")?
            .as_raw();

        // SAFETY: SDL supplies valid handles for the live owned Window. This
        // constructor immediately stores that Window with the resulting surface,
        // and Window retains the VideoSubsystem that owns the display connection.
        // The private fields drop surface before instance before window; neither
        // the surface nor its textures escape the public renderer. The Rc marker
        // prevents moving or sharing this owner across threads. Both initial
        // creation and recreation run here with an instance created without an
        // external display handle, so there is no mismatched display provider.
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(raw_display_handle),
                raw_window_handle,
            })
        }
        .context("creating SDL graphics surface")?;

        Ok(Self {
            surface,
            instance,
            window,
            _main_thread: PhantomData,
        })
    }

    pub(crate) fn instance(&self) -> &wgpu::Instance {
        &self.instance
    }

    pub(crate) fn surface(&self) -> &wgpu::Surface<'_> {
        &self.surface
    }

    pub(crate) fn window(&self) -> &Window {
        &self.window
    }

    pub(crate) fn recreate(&mut self) -> Result<()> {
        // Reuse the instance so the renderer's adapter and device remain valid.
        // SDL Window clones retain the same native window and video subsystem;
        // replacing this owner drops the previous surface before either clone.
        let replacement = Self::from_parts(self.instance.clone(), self.window.clone())
            .context("recreating lost SDL graphics surface")?;
        *self = replacement;
        Ok(())
    }
}
