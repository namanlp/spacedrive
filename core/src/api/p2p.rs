use rspc::{alpha::AlphaRouter, ErrorCode};
use sd_p2p::spacetunnel::RemoteIdentity;
use serde::Deserialize;
use specta::Type;
use std::path::PathBuf;
use uuid::Uuid;

use crate::p2p::{operations, P2PEvent, PairingDecision};

use super::{Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("events", {
			R.subscription(|node, _: ()| async move {
				let mut rx = node.p2p.subscribe();

				let mut queued = Vec::new();

				// TODO: Don't block subscription start
				for peer in node.p2p.node.get_discovered() {
					queued.push(P2PEvent::DiscoveredPeer {
						identity: peer.identity,
						metadata: peer.metadata,
					});
				}

				// TODO: Don't block subscription start
				for identity in node
					.p2p
					.manager
					.get_connected_peers()
					.await
					.map_err(|_err| {
						rspc::Error::new(
							ErrorCode::InternalServerError,
							"todo: error getting connected peers".into(),
						)
					})? {
					queued.push(P2PEvent::ConnectedPeer { identity });
				}

				Ok(async_stream::stream! {
					for event in queued.drain(..queued.len()) {
						yield event;
					}

					while let Ok(event) = rx.recv().await {
						yield event;
					}
				})
			})
		})
		// TODO: This has a potentially invalid map key and Specta don't like that. Can bring back in another PR.
		// .procedure("state", {
		// 	R.query(|node, _: ()| async move { Ok(node.p2p.state()) })
		// })
		.procedure("spacedrop", {
			#[derive(Type, Deserialize)]
			pub struct SpacedropArgs {
				identity: RemoteIdentity,
				file_path: Vec<String>,
			}

			R.mutation(|node, args: SpacedropArgs| async move {
				operations::spacedrop(
					node.p2p.clone(),
					args.identity,
					args.file_path
						.into_iter()
						.map(PathBuf::from)
						.collect::<Vec<_>>(),
				)
				.await
				.map_err(|_err| {
					rspc::Error::new(ErrorCode::InternalServerError, "todo: error".into())
				})
			})
		})
		.procedure("acceptSpacedrop", {
			R.mutation(|node, (id, path): (Uuid, Option<String>)| async move {
				match path {
					Some(path) => node.p2p.accept_spacedrop(id, path).await,
					None => node.p2p.reject_spacedrop(id).await,
				};

				Ok(())
			})
		})
		.procedure("cancelSpacedrop", {
			R.mutation(|node, id: Uuid| async move { Ok(node.p2p.cancel_spacedrop(id).await) })
		})
		.procedure("pair", {
			R.mutation(|node, id: RemoteIdentity| async move {
				Ok(node.p2p.pairing.clone().originator(id, node).await)
			})
		})
		.procedure("pairingResponse", {
			R.mutation(|node, (pairing_id, decision): (u16, PairingDecision)| {
				node.p2p.pairing.decision(pairing_id, decision);
				Ok(())
			})
		})
}
