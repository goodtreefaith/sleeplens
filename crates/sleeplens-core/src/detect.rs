use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Nightfile {
    pub date: String,
    pub path: String,
    pub folder: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct As2xDetection {
    pub detected: bool,
    pub serial: Option<String>,
    pub nights: Vec<Nightfile>,
    pub bkp: Option<String>,
    pub months: Vec<String>,
}
/// Recognise AS2X from relative names alone. Accepts either path separator.
pub fn detect_as2x<S: AsRef<str>>(file_names: &[S]) -> As2xDetection {
    let mut nights = Vec::new();
    let mut serials: Vec<(String, usize)> = Vec::new();
    let mut months = std::collections::BTreeSet::new();
    let mut bkp: Option<String> = None;
    let mut bkp_serial: Option<String> = None;
    for path in file_names {
        let path = path.as_ref();
        let parts: Vec<_> = path.split(['/', '\\']).filter(|s| !s.is_empty()).collect();
        let Some(name) = parts.last() else { continue };
        if name.to_ascii_lowercase().ends_with(".bkp") && parts.len() <= 2 {
            let base = &name[..name.len() - 4];
            if !base.is_empty()
                && bkp.as_ref().is_none_or(|p| {
                    parts.len() < p.split(['/', '\\']).filter(|s| !s.is_empty()).count()
                })
            {
                bkp = Some(path.to_owned());
                bkp_serial = Some(base.to_owned());
            }
            continue;
        }
        if name.len() != 12
            || !name.to_ascii_lowercase().ends_with(".dat")
            || !name.as_bytes()[..8].iter().all(u8::is_ascii_digit)
            || parts.len() < 2
        {
            continue;
        }
        let folder = parts[parts.len() - 2];
        let month = &name[..6];
        if !folder.ends_with(month) || folder.len() == month.len() {
            continue;
        }
        let serial = &folder[..folder.len() - month.len()];
        if let Some((_, count)) = serials.iter_mut().find(|(s, _)| s == serial) {
            *count += 1;
        } else {
            serials.push((serial.to_owned(), 1));
        }
        months.insert(month.to_owned());
        nights.push(Nightfile {
            date: name[..8].to_owned(),
            path: path.to_owned(),
            folder: folder.to_owned(),
        });
    }
    let serial = serials
        .iter()
        .enumerate()
        .filter(|(_, (s, _))| bkp_serial.as_ref().is_some_and(|b| b.ends_with(s)))
        .max_by_key(|(i, (_, n))| (*n, std::cmp::Reverse(*i)))
        .map(|(_, item)| item)
        .or_else(|| {
            serials
                .iter()
                .enumerate()
                .max_by_key(|(i, (_, n))| (*n, std::cmp::Reverse(*i)))
                .map(|(_, item)| item)
        })
        .map(|(s, _)| s.clone());
    nights.retain(|n| serial.as_ref().is_some_and(|s| n.folder.starts_with(s)));
    nights.sort_by(|a, b| a.date.cmp(&b.date));
    let bkp = if serial
        .as_ref()
        .zip(bkp_serial.as_ref())
        .is_some_and(|(s, b)| b.ends_with(s))
    {
        bkp
    } else {
        None
    };
    As2xDetection {
        detected: !nights.is_empty(),
        serial,
        nights,
        bkp,
        months: months.into_iter().collect(),
    }
}
