use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

pub enum Node<'a> {
    Open(&'a BytesStart<'a>),
    Close(&'a str),
    Text(&'a str),
}

pub fn walk(xml: &str, mut visit: impl FnMut(Node)) {
    let mut reader = Reader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => visit(Node::Open(&start)),
            Ok(Event::Empty(start)) => {
                visit(Node::Open(&start));
                visit(Node::Close(start.name().0));
            }
            Ok(Event::End(end)) => visit(Node::Close(end.name().0)),
            Ok(Event::Text(text)) => visit(Node::Text(&text.xml10_content())),
            Ok(Event::GeneralRef(entity)) => {
                if let Some(text) = entity_text(&entity.xml10_content(), entity.resolve_char_ref().ok().flatten()) {
                    visit(Node::Text(&text));
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
    }
}

pub fn name<'a>(start: &'a BytesStart) -> &'a str {
    start.name().into_inner()
}

pub fn attr(start: &BytesStart, key: &str) -> Option<String> {
    let found = start.try_get_attribute(key).ok()??;
    found.normalized_value(XmlVersion::Implicit1_0).ok().map(|v| v.into_owned())
}

fn entity_text(name: &str, char_ref: Option<char>) -> Option<String> {
    if let Some(c) = char_ref {
        return Some(c.to_string());
    }
    let text = match name {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        _ => return None,
    };
    Some(text.to_string())
}
