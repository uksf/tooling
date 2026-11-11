use crate::odol::Model;
use serde_json::Value;

fn string<'a>(op: &'a Value, field: &str) -> Result<&'a str, String> {
    op.get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("op is missing string \"{field}\": {op}"))
}

pub fn set_skeleton_name(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    let mut model = Model::parse(data).map_err(|e| format!("{label}: {e}"))?;
    if model.write()? != data {
        return Err(format!(
            "{label}: the ODOL library does not re-serialise this model byte-identically; refusing to patch it"
        ));
    }
    let from = string(op, "from")?;
    let to = string(op, "to")?;
    if to.is_empty() {
        return Err(format!("{label}: set-skeleton-name \"to\" is empty"));
    }
    let skeleton = &mut model.info.skeleton;
    if !crate::op_json::same(&skeleton.name, from) {
        return Err(format!(
            "{label}: skeleton is '{}', the patch expects '{from}'",
            String::from_utf8_lossy(&skeleton.name)
        ));
    }
    skeleton.name = crate::op_json::latin1_lossy(to);
    let result = model.write()?;
    Model::parse(&result).map_err(|e| format!("{label}: {e}"))?;
    Ok(result)
}
