use log::debug;
use wruster::http::Request;
use wruster::router::HttpHandler;

pub(crate) fn log(handler: HttpHandler) -> HttpHandler {
    Box::new(move |request: &mut Request| {
        debug!("request {:?}", request);
        let response = handler(request);
        debug!("response {:?}", response);
        response
    })
}
