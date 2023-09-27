
use std::{path::Path, str::FromStr, fs::File, io::Write};


use hyper::{Uri, client, body::HttpBody};
use hyper_rustls::ConfigBuilderExt;

use super::{DownloadBackend, DownloadError, Callback};



#[derive(Clone, Copy)]
pub struct HyperBackend {}

impl DownloadBackend for HyperBackend {
    fn download(
        &self,
        remote_path: &str,
        local_path: &Path,
        callback: &mut dyn Callback,
    ) -> Result<(), DownloadError> {
        self.async_download(remote_path, local_path, callback)
    }
}

impl HyperBackend {
    #[tokio::main]
    async fn async_download(
        &self,
        remote_path: &str,
        local_path: &Path,
        callback: &mut dyn Callback,
    ) -> Result<(), DownloadError> {
        
        
        // First parameter is target URL (mandatory).
        let url = Uri::from_str(remote_path).unwrap();

        // Prepare the TLS client config
        let tls = rustls::ClientConfig::builder()
                .with_safe_defaults()
                .with_webpki_roots()
                //.with_native_roots()
                .with_no_client_auth();
        
        // Prepare the HTTPS connector
        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_tls_config(tls)
            .https_or_http()
            .enable_http1()
            .build();

        // Build the hyper client from the HTTPS connector.
        let client: client::Client<_, hyper::Body> = client::Client::builder().build(https);

        // Prepare a chain of futures which sends a GET request, inspects
        // the returned headers, collects the whole body and prints it to
        // stdout.

        let mut res = client
            .get(url)
            .await.unwrap();

        //println!("Status:\n{}", res.status());
        //println!("Headers:\n{:#?}", res.headers());


        let mut output = File::create(local_path)?;
        let mut offset = 0;
        let len = res.size_hint().upper().unwrap_or_default();

        callback.start(len, remote_path);

        while let Some(next) = res.data().await {
            let chunk = next.unwrap().to_vec();
            output.write_all(&chunk)?;

            offset += chunk.len();

            callback.update(offset);
        }

        /*let body = res.into_body();
        

        let bytes = to_bytes(body)
            .await.unwrap();


        callback.start(len, remote_path);

        for byte in bytes {
            
            //println!("test {offset}");
            callback.update(offset + 1);

            output.write_all(&[byte]).unwrap();
            //output.write_at(&[byte], offset)?;
            offset += 1;

        }*/
        //println!("Body:\n{}", String::from_utf8_lossy(&body));

        Ok(())
    }
}
