// Copyright (C) 2016  Max Planck Institute for Human Development
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use iron::{Request, Response, IronResult};
use iron::status;
use iron::mime::Mime;
use persistent::Read;
use chrono::{UTC, TimeZone};
use treexml::{Document, Element};
use ::web::server::ConnectionPoolKey;
use ::web::backend::db::PackageVersion;
use ::web::backend::xml::ToNugetFeedXml;

pub fn packages(req: &mut Request) -> IronResult<Response> {
    let base_url = {
        let url = &req.url;
        if (&*url.scheme == "http" && url.port == 80) || (&*url.scheme == "https" && url.port == 443) {
            format!("{}://{}", url.scheme, url.host)
        } else {
            format!("{}://{}:{}", url.scheme, url.host, url.port)
        }
    };
    let connection_pool = req.extensions.get::<Read<ConnectionPoolKey>>().unwrap();

    let connection = match connection_pool.get() {
        Ok(connection) => connection,
        Err(err) => {
            error!("{:?}", err);
            return Ok(Response::with((status::InternalServerError, "Database Error, please try again later")));
        }
    };

    //TODO limit packages when we reach a high count and add helper for quicker updated retrieval
    let packages = match PackageVersion::all(&*connection) {
        Ok(packages) => packages,
        Err(err) => {
            error!("{:?}", err);
            return Ok(Response::with((status::InternalServerError, "Database Error, please try again later")));
        }
    };

    let mut feed = Element::new("feed");
    feed.attributes.insert(String::from("xml:base"), format!("{}/api/v2/", &*base_url));
    feed.attributes.insert(String::from("xmlns:d"), String::from("http://schemas.microsoft.com/ado/2007/08/dataservices"));
    feed.attributes.insert(String::from("xmlns:m"), String::from("http://schemas.microsoft.com/ado/2007/08/dataservices/metadata"));
    feed.attributes.insert(String::from("xmlns"), String::from("http://www.w3.org/2005/Atom"));

    let mut title = Element::new("title");
    title.text = Some(String::from("Packages"));
    feed.children.push(title);

    let mut id = Element::new("id");
    id.text = Some(format!("{}/api/v2/Packages", &*base_url));
    feed.children.push(id);

    let mut last_updated = UTC.ymd(1900, 1, 1).and_hms(0, 0, 0).naive_utc();
    for pkg in packages.iter()
    {
        if pkg.last_updated() > &last_updated {
            last_updated = pkg.last_updated().clone();
        }
    }
    let mut updated = Element::new("updated");
    updated.text = Some(format!("{:?}Z", last_updated));
    feed.children.push(updated);

    let mut link = Element::new("link");
    link.attributes.insert(String::from("title"), String::from("Packages"));
    link.attributes.insert(String::from("href"), String::from("Packages"));
    feed.children.push(link);

    for pkg in packages.into_iter()
    {
        feed.children.push(match pkg.xml_entry(&*base_url, &*connection) {
            Ok(entry) => entry,
            Err(err) => {
                error!("{:?}", err);
                return Ok(Response::with((status::InternalServerError, "Database Error, please try again later")));
            }
        });
    }

    //TODO when we limit we need to generate a continuation link, also we need to map this in the server
    //e.g. <link rel="next" href="http://chocolatey.org/api/v2/Packages?$skiptoken='1password','1.0.9.332'" />

    let document = Document{
       root: Some(feed),
       .. Document::default()
    };

    Ok(Response::with((status::Ok, format!("{}", document), {
        let mime: Mime = "application/atom+xml".parse().unwrap();
        mime
    })))
}
