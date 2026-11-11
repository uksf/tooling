pub fn resolution(name: &str) -> Result<f32, String> {
    let value = match name.to_ascii_lowercase().as_str() {
        "geometry" => 1e13,
        "geometrysimple" => 2e13,
        "geometryphysold" => 3e13,
        "geometryphys" => 4e13,
        "viewpilot" => 1100.0,
        "memory" => 1e15,
        "landcontact" => 2e15,
        "roadway" => 3e15,
        "paths" => 4e15,
        "hitpoints" => 5e15,
        "geometryview" => 6e15,
        "geometryfire" => 7e15,
        "viewgeometrycargo" => 8e15,
        "viewgeometrypilot" => 1.3e16,
        "viewgeometrygunner" => 1.5e16,
        _ => {
            let n = crate::dotnet_parse::parse_float(name)
                .ok_or_else(|| format!("unknown LOD name '{name}'"))?;
            if !(0.0..1e13).contains(&n) {
                return Err(format!("unknown LOD name '{name}'"));
            }
            n
        }
    };
    Ok(value)
}
