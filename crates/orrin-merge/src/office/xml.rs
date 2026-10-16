use quick_xml::Reader;
use quick_xml::escape::unescape;
use quick_xml::events::{BytesStart, Event};

pub struct Split<'a> {
    pub head: &'a str,
    pub children: Vec<&'a str>,
    pub tail: &'a str,
}

pub fn split<'a>(xml: &'a str, container: &str) -> Option<Split<'a>> {
    let mut reader = Reader::from_str(xml);
    let mut depth = 0;
    let mut inside = None;
    let mut head = "";
    let mut child_start = 0;
    let mut children = Vec::new();
    loop {
        let start = reader.buffer_position() as usize;
        let event = reader.read_event().ok()?;
        let end = reader.buffer_position() as usize;
        match event {
            Event::Start(element) => {
                depth += 1;
                if inside.is_none() && element.name().as_ref() == container {
                    inside = Some(depth);
                    head = &xml[..end];
                } else if inside == Some(depth - 1) {
                    child_start = start;
                }
            }
            Event::End(_) => {
                if inside == Some(depth) {
                    return Some(Split { head, children, tail: &xml[start..] });
                }
                depth -= 1;
                if inside == Some(depth) {
                    children.push(&xml[child_start..end]);
                }
            }
            Event::Empty(_) | Event::Comment(_) if inside == Some(depth) => children.push(&xml[start..end]),
            Event::Eof => return None,
            _ => {}
        }
    }
}

pub fn join(head: &str, children: impl IntoIterator<Item = impl AsRef<str>>, tail: &str) -> String {
    let mut xml = head.to_string();
    for child in children {
        xml.push_str(child.as_ref());
    }
    xml + tail
}

pub fn retain(xml: &str, container: &str, keep: impl Fn(&str) -> bool) -> String {
    match split(xml, container) {
        Some(split) => join(split.head, split.children.into_iter().filter(|child| keep(child)), split.tail),
        None => xml.to_string(),
    }
}

fn start_tag(element: &str) -> Option<BytesStart<'_>> {
    match Reader::from_str(element).read_event().ok()? {
        Event::Start(start) | Event::Empty(start) => Some(start),
        _ => None,
    }
}

pub fn tag(element: &str) -> Option<String> {
    start_tag(element).map(|start| start.name().as_ref().to_string())
}

pub fn attribute(element: &str, name: &str) -> Option<String> {
    let start = start_tag(element)?;
    let value = start.try_get_attribute(name).ok()??;
    let value = unescape(&value.value).ok()?.into_owned();
    Some(value)
}

pub fn without_attribute(xml: &str, name: &str) -> String {
    let marker = format!(" {name}=\"");
    let Some(start) = xml.find(&marker) else {
        return xml.to_string();
    };
    let value = start + marker.len();
    let end = xml[value..].find('"').map_or(xml.len(), |at| value + at + 1);
    format!("{}{}", &xml[..start], &xml[end..])
}

fn element_span(xml: &str, name: &str) -> Option<(usize, usize)> {
    let open = format!("<{name}");
    let (start, _) = xml.match_indices(&open).find(|(at, _)| xml[at + open.len()..].starts_with([' ', '>', '/']))?;
    let tag_end = start + xml[start..].find('>')? + 1;
    if xml[..tag_end].ends_with("/>") {
        return Some((start, tag_end));
    }
    let close = format!("</{name}>");
    Some((start, tag_end + xml[tag_end..].find(&close)? + close.len()))
}

pub fn without(xml: &str, name: &str) -> String {
    match element_span(xml, name) {
        Some((start, end)) => format!("{}{}", &xml[..start], &xml[end..]),
        None => xml.to_string(),
    }
}

pub fn inner(element: &str) -> &str {
    match (element.find('>'), element.rfind("</")) {
        (Some(open), Some(close)) if open < close => &element[open + 1..close],
        _ => "",
    }
}
