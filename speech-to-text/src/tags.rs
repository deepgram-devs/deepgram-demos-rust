use std::io;

pub(crate) fn parse(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_owned)
        .collect()
}

pub(crate) fn validate(tags: &[String]) -> Result<(), io::Error> {
    for (index, tag) in tags.iter().enumerate() {
        let character_count = tag.chars().count();
        if character_count > 128 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Deepgram tags must be 128 characters or fewer (tag {} has {character_count})",
                    index + 1
                ),
            ));
        }
    }

    Ok(())
}

pub(crate) fn append_query_params(params: &mut Vec<String>, tags: &[String]) {
    for tag in tags {
        params.push(format!("tag={}", urlencoding::encode(tag)));
    }
}
