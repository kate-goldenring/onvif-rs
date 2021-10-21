use onvif::{schema, soap};
use structopt::StructOpt;
use tracing::debug;
use url::Url;

#[derive(StructOpt)]
#[structopt(name = "provisioning", about = "ONVIF camera provisioning tool")]
struct Args {
    #[structopt(global = true, long, requires = "password")]
    username: Option<String>,

    #[structopt(global = true, long, requires = "username")]
    password: Option<String>,

    /// The device's base URI, typically just to the HTTP root.
    /// The service-specific path (such as `/onvif/device_support`) will be appended to this.
    // Note this is an `Option` because global options can't be required in clap.
    // https://github.com/clap-rs/clap/issues/1546
    #[structopt(global = true, long)]
    uri: Option<Url>,

    #[structopt(subcommand)]
    cmd: Cmd,
}

#[derive(StructOpt)]
#[structopt()]
enum Cmd {
    GetSystemDateAndTime,
    GetServiceCapabilities,
    PanMove,
    UpgradeSystemFirmware,
}

struct Clients {
    provisioning: soap::client::Client,
    devicemgmt: soap::client::Client,
    event: Option<soap::client::Client>,
    deviceio: Option<soap::client::Client>,
    media: Option<soap::client::Client>,
    media2: Option<soap::client::Client>,
    imaging: Option<soap::client::Client>,
    ptz: Option<soap::client::Client>,
    analytics: Option<soap::client::Client>,
}

impl Clients {
    async fn new(args: &Args) -> Result<Self, String> {
        let creds = match (args.username.as_ref(), args.password.as_ref()) {
            (Some(username), Some(password)) => Some(soap::client::Credentials {
                username: username.clone(),
                password: password.clone(),
            }),
            (None, None) => None,
            _ => panic!("username and password must be specified together"),
        };
        let base_uri = args
            .uri
            .as_ref()
            .ok_or_else(|| "--uri must be specified.".to_string())?;
        let devicemgmt_uri = base_uri.join("onvif/device_service").unwrap();
        let mut out = Self {
            provisioning: soap::client::ClientBuilder::new(&devicemgmt_uri)
                .credentials(creds.clone())
                .build(),
            devicemgmt: soap::client::ClientBuilder::new(&devicemgmt_uri)
                .credentials(creds.clone())
                .build(),
            imaging: None,
            ptz: None,
            event: None,
            deviceio: None,
            media: None,
            media2: None,
            analytics: None,
        };
        let services =
        schema::devicemgmt::get_services(&out.devicemgmt, &Default::default()).await?;
        for s in &services.service {
            let url = Url::parse(&s.x_addr).map_err(|e| e.to_string())?;
            if !url.as_str().starts_with(base_uri.as_str()) {
                return Err(format!(
                    "Service URI {} is not within base URI {}",
                    &s.x_addr, &base_uri
                ));
            }
            let svc = Some(
                soap::client::ClientBuilder::new(&url)
                    .credentials(creds.clone())
                    .build(),
            );
            match s.namespace.as_str() {
                "http://www.onvif.org/ver10/provisioning/wsdl" => {
                    // if s.x_addr != devicemgmt_uri.as_str() {
                    //     return Err(format!(
                    //         "advertised device mgmt uri {} not expected {}",
                    //         &s.x_addr, &devicemgmt_uri
                    //     ));
                    // }
                }
                "http://www.onvif.org/ver10/events/wsdl" => out.event = svc,
                "http://www.onvif.org/ver10/deviceIO/wsdl" => out.deviceio = svc,
                "http://www.onvif.org/ver10/media/wsdl" => out.media = svc,
                "http://www.onvif.org/ver20/media/wsdl" => out.media2 = svc,
                "http://www.onvif.org/ver20/imaging/wsdl" => out.imaging = svc,
                "http://www.onvif.org/ver20/ptz/wsdl" => out.ptz = svc,
                "http://www.onvif.org/ver20/analytics/wsdl" => out.analytics = svc,
                _ => debug!("unknown service: {:?}", s),
            }
        }
        Ok(out)
    }
}

async fn get_system_date_and_time(clients: &Clients) {
    let date =
        schema::devicemgmt::get_system_date_and_time(&clients.devicemgmt, &Default::default())
            .await;
    println!("{:#?}", date);
}

async fn upgrade_system_firmware(clients: &Clients) {
    use crate::schema::validate::Validate;
    let content_type = schema::xmlmime::ContentType("000000".to_string());
    content_type.validate().unwrap();
    let request = schema::devicemgmt::UpgradeSystemFirmware { firmware: schema::onvif::AttachmentData::default()};
    let res =
        schema::devicemgmt::upgrade_system_firmware(&clients.devicemgmt, &request).await;
    println!("res is {:#?}", res);
}

async fn pan_move(clients: &Clients) {
    let service_capabilities = schema::provisioning::get_service_capabilities(&clients.provisioning, &Default::default()).await.unwrap();
    let sources = service_capabilities.capabilities.source;
    if sources.is_empty() {
        println!("No service capabilities");
        return;
    } else {
        schema::provisioning::pan_move(
            &clients.provisioning,
            &schema::provisioning::PanMove { video_source: schema::onvif::ReferenceToken(sources[0].video_source_token.0.clone()), direction: schema::provisioning::PanDirection::Left, timeout: None},
        )
        .await
        .unwrap();
    }

}

// async fn set_imaging_settings(clients: &Clients) {
//     let service_capabilities = schema::imaging::get_service_capabilities(&clients.imaging, &Default::default()).await.unwrap();
//     let sources = service_capabilities.capabilities.source;
//     if sources.is_empty() {
//         println!("No service capabilities");
//         return;
//     } else {
//         schema::provisioning::pan_move(
//             &clients.provisioning,
//             &schema::provisioning::PanMove { video_source: schema::onvif::ReferenceToken(sources[0].video_source_token.0.clone()), direction: schema::provisioning::PanDirection::Left, timeout: None},
//         )
//         .await
//         .unwrap();
//     }

// }

async fn get_service_capabilities(clients: &Clients) {
    match schema::provisioning::get_service_capabilities(&clients.provisioning, &Default::default()).await {
        Ok(capability) => println!("provisioning: {:#?}", capability),
        Err(error) => println!("Failed to fetch provisioning: {}", error.to_string()),
    }

    match schema::devicemgmt::get_service_capabilities(&clients.devicemgmt, &Default::default()).await {
        Ok(capability) => println!("devicemgmt: {:#?}", capability),
        Err(error) => println!("Failed to fetch devicemgmt: {}", error.to_string()),
    }

    if let Some(ref event) = clients.event {
        match schema::event::get_service_capabilities(event, &Default::default()).await {
            Ok(capability) => println!("event: {:#?}", capability),
            Err(error) => println!("Failed to fetch event: {}", error.to_string()),
        }
    }
    if let Some(ref deviceio) = clients.deviceio {
        match schema::deviceio::get_service_capabilities(deviceio, &Default::default()).await {
            Ok(capability) => println!("deviceio: {:#?}", capability),
            Err(error) => println!("Failed to fetch deviceio: {}", error.to_string()),
        }
    }
    if let Some(ref media) = clients.media {
        match schema::media::get_service_capabilities(media, &Default::default()).await {
            Ok(capability) => println!("media: {:#?}", capability),
            Err(error) => println!("Failed to fetch media: {}", error.to_string()),
        }
    }
    if let Some(ref media2) = clients.media2 {
        match schema::media2::get_service_capabilities(media2, &Default::default()).await {
            Ok(capability) => println!("media2: {:#?}", capability),
            Err(error) => println!("Failed to fetch media2: {}", error.to_string()),
        }
    }
    if let Some(ref imaging) = clients.imaging {
        match schema::imaging::get_service_capabilities(imaging, &Default::default()).await {
            Ok(capability) => println!("imaging: {:#?}", capability),
            Err(error) => println!("Failed to fetch imaging: {}", error.to_string()),
        }
    }
    if let Some(ref ptz) = clients.ptz {
        match schema::ptz::get_service_capabilities(ptz, &Default::default()).await {
            Ok(capability) => println!("ptz: {:#?}", capability),
            Err(error) => println!("Failed to fetch ptz: {}", error.to_string()),
        }
    }
    if let Some(ref analytics) = clients.analytics {
        match schema::analytics::get_service_capabilities(analytics, &Default::default()).await {
            Ok(capability) => println!("analytics: {:#?}", capability),
            Err(error) => println!("Failed to fetch analytics: {}", error.to_string()),
        }
    }
}


#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let args = Args::from_args();
    let clients = Clients::new(&args).await.unwrap();

    match args.cmd {
        Cmd::GetSystemDateAndTime => get_system_date_and_time(&clients).await,
        Cmd::PanMove => pan_move(&clients).await,
        // Cmd::GetCapabilities => get_capabilities(&clients).await,
        Cmd::GetServiceCapabilities => get_service_capabilities(&clients).await,
        Cmd::UpgradeSystemFirmware => upgrade_system_firmware(&clients).await,
        // Cmd::GetStreamUris => get_stream_uris(&clients).await,
        // Cmd::GetHostname => get_hostname(&clients).await,
        // Cmd::SetHostname { hostname } => set_hostname(&clients, hostname).await,
        // Cmd::GetDeviceInformation => get_device_information(&clients).await,
        // Cmd::EnableAnalytics => enable_analytics(&clients).await,
        // Cmd::GetAnalytics => get_analytics(&clients).await,
        // Cmd::GetStatus => get_status(&clients).await,
        // Cmd::PanMove => pan_move(&clients).await,
        // Cmd::GetAll => {
        //     get_system_date_and_time(&clients).await;
        //     get_capabilities(&clients).await;
        //     get_service_capabilities(&clients).await;
        //     get_stream_uris(&clients).await;
        //     get_hostname(&clients).await;
        //     get_analytics(&clients).await;
        //     get_status(&clients).await;
        //     pan_move(&clients).await;
        // }
    }
}
