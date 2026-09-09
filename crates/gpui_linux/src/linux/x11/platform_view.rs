//! X11 native-child hosting. The clipping window crops both pixels and input
//! without changing the child's viewport. Native children sit above the GPU
//! surface; X11 does not provide GPUI scene-overlay compositing here.
use gpui::{PlatformViewHandle, PlatformViewUpdate, platform_view_physical_bounds};
use x11rb::{
    connection::Connection,
    protocol::xproto::{self, ConfigureWindowAux, ConnectionExt, CreateWindowAux},
    xcb_ffi::XCBConnection,
};

#[derive(Default)]
pub(super) struct NativeChildren(Vec<Child>);

struct Child {
    handle: PlatformViewHandle,
    clip: u32,
    parent: u32,
    geometry: xproto::GetGeometryReply,
    mapped: bool,
}

impl NativeChildren {
    pub fn update(
        &mut self,
        connection: &XCBConnection,
        parent: u32,
        scale: f32,
        update: &PlatformViewUpdate,
    ) -> anyhow::Result<()> {
        for id in &update.detached {
            if let Some(index) = self.0.iter().position(|child| child.handle.id() == *id) {
                self.detach(connection, index)?;
            }
        }
        for placement in &update.placements {
            let Some(window) = placement.handle.as_x11_window() else {
                continue;
            };
            let bounds = platform_view_physical_bounds(placement.bounds, scale);
            let clip_bounds = platform_view_physical_bounds(placement.clip_bounds(), scale);
            let index = match self
                .0
                .iter()
                .position(|child| child.handle.id() == placement.handle.id())
            {
                Some(index) => index,
                None => {
                    let geometry = connection.get_geometry(window)?.reply()?;
                    let old_parent = connection.query_tree(window)?.reply()?.parent;
                    let mapped = connection.get_window_attributes(window)?.reply()?.map_state
                        != xproto::MapState::UNMAPPED;
                    let clip = connection.generate_id()?;
                    connection
                        .create_window(
                            x11rb::COPY_DEPTH_FROM_PARENT,
                            clip,
                            parent,
                            0,
                            0,
                            1,
                            1,
                            0,
                            xproto::WindowClass::INPUT_OUTPUT,
                            x11rb::COPY_FROM_PARENT,
                            &CreateWindowAux::new(),
                        )?
                        .check()?;
                    if let Err(error) = connection.reparent_window(window, clip, 0, 0)?.check() {
                        connection.destroy_window(clip)?.check()?;
                        return Err(error.into());
                    }
                    self.0.push(Child {
                        handle: placement.handle.clone(),
                        clip,
                        parent: old_parent,
                        geometry,
                        mapped,
                    });
                    self.0.len() - 1
                }
            };
            let child = &self.0[index];
            if clip_bounds.size.width.0 <= 0 || clip_bounds.size.height.0 <= 0 {
                connection.unmap_window(child.clip)?.check()?;
                continue;
            }
            connection
                .configure_window(
                    child.clip,
                    &ConfigureWindowAux::new()
                        .x(clip_bounds.origin.x.0)
                        .y(clip_bounds.origin.y.0)
                        .width(clip_bounds.size.width.0 as u32)
                        .height(clip_bounds.size.height.0 as u32)
                        .stack_mode(xproto::StackMode::ABOVE),
                )?
                .check()?;
            connection
                .configure_window(
                    window,
                    &ConfigureWindowAux::new()
                        .x(bounds.origin.x.0 - clip_bounds.origin.x.0)
                        .y(bounds.origin.y.0 - clip_bounds.origin.y.0)
                        .width(bounds.size.width.0.max(1) as u32)
                        .height(bounds.size.height.0.max(1) as u32),
                )?
                .check()?;
            placement.handle.notify_x11_resize(bounds.size);
            connection.map_window(window)?.check()?;
            connection.map_window(child.clip)?.check()?;
        }
        connection.flush()?;
        Ok(())
    }

    fn detach(&mut self, connection: &XCBConnection, index: usize) -> anyhow::Result<()> {
        let child = &self.0[index];
        let window = child
            .handle
            .as_x11_window()
            .expect("only native X11 children are attached");
        connection.unmap_window(child.clip)?.check()?;
        connection.unmap_window(window)?.check()?;
        connection
            .reparent_window(window, child.parent, child.geometry.x, child.geometry.y)?
            .check()?;
        connection
            .configure_window(
                window,
                &ConfigureWindowAux::new()
                    .width(u32::from(child.geometry.width))
                    .height(u32::from(child.geometry.height)),
            )?
            .check()?;
        // Restore the original visibility, which hosts should initialize hidden.
        if child.mapped {
            connection.map_window(window)?.check()?;
        }
        connection.destroy_window(child.clip)?.check()?;
        self.0.remove(index);
        Ok(())
    }

    pub fn clear(&mut self, connection: &XCBConnection) -> anyhow::Result<()> {
        while !self.0.is_empty() {
            self.detach(connection, self.0.len() - 1)?;
        }
        connection.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Bounds, PlatformViewPlacement, point, px, size};
    use std::{cell::Cell, rc::Rc};

    #[test]
    #[ignore = "requires an X11 server: xvfb-run cargo test -p gpui-box-linux x11_native_children -- --ignored"]
    fn x11_native_children_clip_resize_stack_detach_and_retain() -> anyhow::Result<()> {
        let (connection, screen) = XCBConnection::connect(None)?;
        let root = connection.setup().roots[screen].root;
        let create = |parent| -> anyhow::Result<u32> {
            let id = connection.generate_id()?;
            connection
                .create_window(
                    x11rb::COPY_DEPTH_FROM_PARENT,
                    id,
                    parent,
                    7,
                    11,
                    300,
                    250,
                    0,
                    xproto::WindowClass::INPUT_OUTPUT,
                    x11rb::COPY_FROM_PARENT,
                    &CreateWindowAux::new(),
                )?
                .check()?;
            Ok(id)
        };
        let parent = create(root)?;
        connection.map_window(parent)?.check()?;
        let first = create(parent)?;
        let second = create(parent)?;
        let owner = Rc::new(());
        let weak = Rc::downgrade(&owner);
        let allocated = Rc::new(Cell::new(size(
            gpui::DevicePixels(0),
            gpui::DevicePixels(0),
        )));
        let observed = allocated.clone();
        let handle = unsafe { PlatformViewHandle::from_x11_window(first) }
            .keep_alive(owner)
            .with_x11_resize_handler(move |size| observed.set(size));
        let other = unsafe { PlatformViewHandle::from_x11_window(second) };
        let bounds = Bounds::new(point(px(-20.), px(10.)), size(px(200.), px(120.)));
        let clip = Bounds::new(point(px(5.), px(25.)), size(px(130.), px(80.)));
        let placement = PlatformViewPlacement::new(handle.clone(), bounds, clip);
        let second_placement = PlatformViewPlacement::new(other.clone(), bounds, clip);
        let mut host = NativeChildren::default();
        host.update(
            &connection,
            parent,
            1.5,
            &PlatformViewUpdate {
                placements: vec![placement.clone(), second_placement.clone()],
                detached: vec![],
            },
        )?;
        let child = connection.get_geometry(first)?.reply()?;
        let clip_id = connection.query_tree(first)?.reply()?.parent;
        let clip_geometry = connection.get_geometry(clip_id)?.reply()?;
        assert_eq!(
            (child.width, child.height, child.x, child.y),
            (300, 180, -38, -23)
        );
        assert_eq!(
            (
                clip_geometry.x,
                clip_geometry.y,
                clip_geometry.width,
                clip_geometry.height
            ),
            (8, 38, 195, 120)
        );
        assert_eq!(
            allocated.get(),
            size(gpui::DevicePixels(300), gpui::DevicePixels(180))
        );
        assert_eq!(
            connection.query_tree(parent)?.reply()?.children.last(),
            Some(&host.0[1].clip)
        );
        // Hit testing belongs to the clip parent, not the full viewport.
        connection
            .warp_pointer(x11rb::NONE, parent, 0, 0, 0, 0, 10, 40)?
            .check()?;
        assert_eq!(
            connection.query_pointer(parent)?.reply()?.child,
            host.0[1].clip
        );
        connection
            .warp_pointer(x11rb::NONE, parent, 0, 0, 0, 0, 3, 20)?
            .check()?;
        assert_eq!(
            connection.query_pointer(parent)?.reply()?.child,
            x11rb::NONE
        );
        host.update(
            &connection,
            parent,
            2.,
            &PlatformViewUpdate {
                placements: vec![second_placement.clone(), placement.clone()],
                detached: vec![],
            },
        )?;
        assert_eq!(
            allocated.get(),
            size(gpui::DevicePixels(400), gpui::DevicePixels(240))
        );
        assert_eq!(
            connection.query_tree(parent)?.reply()?.children.last(),
            Some(&clip_id)
        );
        drop(placement);
        drop(handle);
        assert!(
            weak.upgrade().is_some(),
            "native host retains controller through detach"
        );
        host.clear(&connection)?;
        assert!(weak.upgrade().is_none());
        assert_eq!(connection.query_tree(first)?.reply()?.parent, parent);
        assert_eq!(
            connection.get_window_attributes(first)?.reply()?.map_state,
            xproto::MapState::UNMAPPED
        );
        let restored = connection.get_geometry(first)?.reply()?;
        assert_eq!(
            (restored.x, restored.y, restored.width, restored.height),
            (7, 11, 300, 250)
        );
        connection.destroy_window(parent)?.check()?;
        Ok(())
    }
}
