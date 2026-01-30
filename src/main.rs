// Hi, this is xantarius I just want to make you guys aware of this code first of all for me it was
// very complex to get this even working. The only tip I can give you guys is to read the xml file
// that I have added in the comments, so you can basically undersatnd each event of the interface
//
// second thing, you need to understand how objects working in the wayland, and how request, event
// model works here, get those concepts clear and read the xml file, then it will be easy for you to go through this code
// otherwise honestly nothing will make sense here trust me

use std::sync::Arc;

use wayland_client::{Connection, Dispatch, EventQueue, Proxy, protocol::wl_registry};
use wayland_protocols_wlr::output_management::v1::client::{
    zwlr_output_head_v1, zwlr_output_manager_v1, zwlr_output_mode_v1,
};

#[derive(Debug)]
struct AppState;

// Protocol: https://gitlab.freedesktop.org/wayland/wayland/-/blob/main/protocol/wayland.xml#L71
impl Dispatch<wl_registry::WlRegistry, ()> for AppState {
    fn event(
        _: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &wayland_client::QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
            && interface == zwlr_output_manager_v1::ZwlrOutputManagerV1::interface().name
        {
            registry.bind::<zwlr_output_manager_v1::ZwlrOutputManagerV1, _, _>(
                name,
                version,
                qh,
                (),
            );
        }
    }
}

// Protocol: https://gitlab.freedesktop.org/wlroots/wlr-protocols/-/blob/master/unstable/wlr-output-management-unstable-v1.xml#L46
impl Dispatch<zwlr_output_manager_v1::ZwlrOutputManagerV1, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &zwlr_output_manager_v1::ZwlrOutputManagerV1,
        _: zwlr_output_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &wayland_client::QueueHandle<Self>,
    ) {
    }

    fn event_created_child(
        opcode: u16,
        qh: &wayland_client::QueueHandle<Self>,
    ) -> Arc<dyn wayland_client::backend::ObjectData> {
        if opcode == 0 {
            qh.make_data::<zwlr_output_head_v1::ZwlrOutputHeadV1, _>(())
        } else {
            unreachable!("unknown opcode for zwlr_output_manager_v1")
        }
    }
}

// Protocol: https://gitlab.freedesktop.org/wlroots/wlr-protocols/-/blob/master/unstable/wlr-output-management-unstable-v1.xml#L96
impl Dispatch<zwlr_output_head_v1::ZwlrOutputHeadV1, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &zwlr_output_head_v1::ZwlrOutputHeadV1,
        event: <zwlr_output_head_v1::ZwlrOutputHeadV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &wayland_client::QueueHandle<Self>,
    ) {
        if let zwlr_output_head_v1::Event::Name { name } = event {
            println!("{}", name)
        }
    }

    fn event_created_child(
        opcode: u16,
        qh: &wayland_client::QueueHandle<Self>,
    ) -> Arc<dyn wayland_client::backend::ObjectData> {
        if opcode == 3 {
            qh.make_data::<zwlr_output_mode_v1::ZwlrOutputModeV1, _>(())
        } else {
            unreachable!("unknown opcode for zwlr_output_head_v1")
        }
    }
}

// Protocol: https://gitlab.freedesktop.org/wlroots/wlr-protocols/-/blob/master/unstable/wlr-output-management-unstable-v1.xml#L250
impl Dispatch<zwlr_output_mode_v1::ZwlrOutputModeV1, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &zwlr_output_mode_v1::ZwlrOutputModeV1,
        event: <zwlr_output_mode_v1::ZwlrOutputModeV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            zwlr_output_mode_v1::Event::Refresh { refresh } => {
                println!("{}", refresh);
            }
            zwlr_output_mode_v1::Event::Size { height, width } => {
                println!("height {} width {}", height, width);
            }
            _ => {}
        }
    }
}

fn main() {
    let mut state = AppState {};
    let conn = Connection::connect_to_env().expect("error: failed to connect to wayland server");
    let display_object = conn.display();
    let mut event_queue: EventQueue<AppState> = conn.new_event_queue();
    let queue_handler = event_queue.handle();
    display_object.get_registry(&queue_handler, ());
    event_queue
        .roundtrip(&mut state)
        .expect("error: failed to start the event queue roundtrip");
    loop {
        event_queue
            .blocking_dispatch(&mut state)
            .expect("error: failed to start the dispacth pending event");
    }
}
