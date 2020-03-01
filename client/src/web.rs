#[macro_use]
use stdweb;
use wasm_bindgen::prelude::*;

use stdweb::traits::IMouseEvent;
use stdweb::unstable::TryInto;
use stdweb::web::event::{
    IEvent, IKeyboardEvent, KeyDownEvent, KeyUpEvent, KeyboardLocation, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent,
};
use stdweb::web::html_element::CanvasElement;
use stdweb::web::{self, CanvasRenderingContext2d, IEventTarget, INonElementParentNode};

pub fn main() {
    stdweb::initialize();

    // Retrieve canvas
    let canvas: CanvasElement = web::document()
        .get_element_by_id("puzzleverse")
        .unwrap()
        .try_into()
        .unwrap();

    // Retrieve context
    let ctx: CanvasRenderingContext2d = canvas.get_context().unwrap();

    let client = super::Client::new();
    stdweb::event_loop();
}
