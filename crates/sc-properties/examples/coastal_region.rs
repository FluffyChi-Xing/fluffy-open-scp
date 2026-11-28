//! Build and read back the standalone Half Coast region candidate.
use dbpf::Package;
use sc_properties::region_authoring::{
    CoastalRegionSpec, RegionLocale, RetailSources, build_coastal_region, verify_coastal_region,
};
use std::{error::Error, fs::OpenOptions, io::Write, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let [root, locale, language, output] = args.as_slice() else {
        return Err("usage: coastal_region <SimCityData> <locale/Data.package> <en-US|zh-CN|zh-TW> <out.package>".into());
    };
    let language = match language.as_str() {
        "en-US" => RegionLocale::EnUs,
        "zh-CN" => RegionLocale::ZhCn,
        "zh-TW" => RegionLocale::ZhTw,
        _ => return Err("unsupported locale".into()),
    };
    let open = |name| Package::open(Path::new(root).join(name));
    let sources = RetailSources {
        terrain: &open("SimCity_RegionTerrain0.package")?,
        ep1: &open("SimCityDataEP1.package")?,
        app: &open("SimCity_App.package")?,
        game: &open("SimCity_Game.package")?,
        locale: &Package::open(locale)?,
        language,
    };
    let spec = CoastalRegionSpec::default();
    let bytes = build_coastal_region(&sources, spec)?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(output)?;
    let verify = (|| -> Result<_, Box<dyn Error>> {
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        Ok(verify_coastal_region(&Package::open(output)?, spec)?)
    })();
    let report = match verify {
        Ok(report) => report,
        Err(error) => {
            std::fs::remove_file(output)?;
            return Err(error);
        }
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
