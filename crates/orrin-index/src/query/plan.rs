use regex_syntax::hir::{Hir, HirKind};

use crate::analyse::trigrams;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plan {
    All,
    Grams(Vec<u32>),
    And(Vec<Plan>),
    Or(Vec<Plan>),
}

pub fn literal(text: &str, case_sensitive: bool) -> Plan {
    grams(text.as_bytes(), case_sensitive)
}

pub fn regex(pattern: &str, case_sensitive: bool) -> Plan {
    match regex_syntax::Parser::new().parse(pattern) {
        Ok(hir) => from_hir(&hir, case_sensitive),
        Err(_) => Plan::All,
    }
}

fn grams(bytes: &[u8], case_sensitive: bool) -> Plan {
    let grams = trigrams::of_literal(bytes, case_sensitive);
    if grams.is_empty() { Plan::All } else { Plan::Grams(grams) }
}

fn from_hir(hir: &Hir, case_sensitive: bool) -> Plan {
    match hir.kind() {
        HirKind::Literal(literal) => grams(&literal.0, case_sensitive),
        HirKind::Capture(capture) => from_hir(&capture.sub, case_sensitive),
        HirKind::Repetition(repeat) if repeat.min > 0 => from_hir(&repeat.sub, case_sensitive),
        HirKind::Alternation(options) => any(options.iter().map(|o| from_hir(o, case_sensitive)).collect()),
        HirKind::Concat(parts) => concat(parts, case_sensitive),
        _ => Plan::All,
    }
}

fn concat(parts: &[Hir], case_sensitive: bool) -> Plan {
    let mut plans = Vec::new();
    let mut run = Vec::new();
    for part in parts {
        if let HirKind::Literal(literal) = part.kind() {
            run.extend_from_slice(&literal.0);
            continue;
        }
        plans.push(grams(&run, case_sensitive));
        run.clear();
        plans.push(from_hir(part, case_sensitive));
    }
    plans.push(grams(&run, case_sensitive));
    all(plans)
}

fn all(plans: Vec<Plan>) -> Plan {
    let mut kept: Vec<Plan> = plans.into_iter().filter(|p| *p != Plan::All).collect();
    match kept.len() {
        0 => Plan::All,
        1 => kept.remove(0),
        _ => Plan::And(kept),
    }
}

fn any(plans: Vec<Plan>) -> Plan {
    if plans.is_empty() || plans.contains(&Plan::All) { Plan::All } else { Plan::Or(plans) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regex_keeps_required_literals() {
        assert_eq!(regex(r"\w+", true), Plan::All);
        assert_eq!(regex("ab", true), Plan::All);
        let Plan::And(parts) = regex(r"fn\s+hello_(world|there)", true) else { panic!() };
        assert_eq!(parts.len(), 2);
        assert!(matches!(&parts[1], Plan::Or(options) if options.len() == 2));
    }
}
