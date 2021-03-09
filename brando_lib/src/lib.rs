mod builder;
mod buildings;
mod errors;
mod heigth;
mod map;
mod mayor;
mod orchestrator;
mod point;
mod requests;
mod responses;

use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};

use map::MatrixMap;
use mayor::MainMayor;
use orchestrator::Orchestrator;
use point::Point;
use requests::Request;
use responses::Response;

pub fn brando() -> (Sender<Request>, Receiver<Response>) {
    let dim = Point::new(200, 200);
    let map = MatrixMap::new(dim);
    let mayor = MainMayor::new();

    let mut orchestrator = Orchestrator::new(map, mayor);

    let (tx, rx): (Sender<Request>, Receiver<Request>) = mpsc::channel();
    let (tx_response, rx_response): (Sender<Response>, Receiver<Response>) = mpsc::channel();

    for msg in rx.iter() {
        match msg {
            Request::AddBuildingRequest(req) => {
                let response = orchestrator.add_building(req).unwrap();
                tx_response
                    .send(Response::AddBuildingResponse(response))
                    .unwrap();
            }
            Request::DeleteBuildingRequest(req) => {
                let response = orchestrator.delete_building(req).unwrap();
                tx_response
                    .send(Response::DeleteBuildingResponse(response))
                    .unwrap();
            }
            Request::GetSnapshotRequest(req) => {
                let response = orchestrator.get_map_snapshot(req);
                tx_response
                    .send(Response::GetSnapshotResponse(response))
                    .unwrap();
            }
            Request::Ping => {
                tx_response.send(Response::Pong).unwrap();
            }
            Request::Close => {
                tx_response.send(Response::Close).unwrap();
            }
        }
    }

    return (tx, rx_response);
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
