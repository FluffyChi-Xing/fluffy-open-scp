//! Insert a city scene and standalone stock logo behind the updater controls.

use dbpf::{OverlayEntry, Package, ResourceId};
use serde_json::{Value, json};
use std::{error::Error, fs, path::Path};

const LAYOUT: ResourceId = ResourceId {
    type_id: 0x6777_1F5C,
    group: 0x0B07_4E5A,
    instance: 0xC396_9E96,
};
const PICTURE: ResourceId = ResourceId {
    type_id: 0x2F7D_0004,
    group: 0,
    instance: 0x89C0_DA4B,
};
const STOCK_LOGO: ResourceId = ResourceId {
    instance: 0xE5CB_7D7D,
    ..PICTURE
};
const BACKGROUND_ID: u64 = 244216086;
const LIVE_SCENE_ID: u64 = 0x18AB_51F2;

fn max_instance(node: &Value) -> u64 {
    let own_id = node["instanceID"].as_u64().unwrap_or_default();
    node["children"]
        .as_array()
        .and_then(|children| children.iter().map(max_instance).max())
        .unwrap_or_default()
        .max(own_id)
}

fn image_window(id: u64, bounds: [u16; 4], image: &str, pin: u8) -> Value {
    let [left, top, width, height] = bounds;
    json!({
        "instanceID": id,
        "left": left, "top": top, "width": width, "height": height,
        "horizontalPinType": pin, "verticalPinType": pin,
        "visibility": true, "ignoreMouse": true, "type": "cWindow",
        "drawable": { "type": 2, "images": [image] }
    })
}

pub(super) fn entries(
    stock: &Package,
    picture: &Path,
    live: bool,
) -> Result<[OverlayEntry; 2], Box<dyn Error>> {
    if stock.entry(STOCK_LOGO).is_none() || stock.entry(PICTURE).is_some() {
        return Err("missing stock logo or conflicting custom picture TGI".into());
    }
    let content = fs::read(picture)?;
    image::load_from_memory_with_format(&content, image::ImageFormat::Png)?;

    let source = stock.read(stock.entry(LAYOUT).ok_or("updater layout missing")?)?;
    let mut layout: Value = serde_json::from_slice(&source)?;
    let first_id = max_instance(&layout) + 1;
    let bg = layout["children"]
        .as_array_mut()
        .and_then(|children| {
            children
                .iter_mut()
                .find(|node| node["controlID"].as_u64() == Some(BACKGROUND_ID))
        })
        .ok_or("updater background window missing")?;
    if bg["drawable"]["images"][0].as_str() != Some("Graphics/Launcher/BG-Launcher.png")
        || bg["width"].as_u64() != Some(900)
        || bg["height"].as_u64() != Some(576)
    {
        return Err("unexpected stock updater background window".into());
    }
    let children = bg["children"]
        .as_array_mut()
        .ok_or("updater background children missing")?;
    let mut scene = image_window(
        first_id,
        [3, 3, 894, 484],
        "Graphics/Launcher/OpenSCP-Launcher-Background.png",
        2,
    );
    if live {
        scene["controlID"] = json!(LIVE_SCENE_ID);
    }
    children.splice(
        0..0,
        [
            // Preserve the original border and footer by covering the city area.
            scene,
            image_window(
                first_id + 1,
                [30, 0, 382, 109],
                "Graphics/Common/Graphics/Logo_SimCity-OrangeTM.png",
                0,
            ),
        ],
    );
    Ok([
        OverlayEntry::new(LAYOUT, serde_json::to_vec(&layout)?),
        OverlayEntry::new(PICTURE, content),
    ])
}
