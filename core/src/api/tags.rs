use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use itertools::{Either, Itertools};
use rspc::{alpha::AlphaRouter, ErrorCode};
use sd_cache::{CacheNode, Normalise, NormalisedResult, NormalisedResults, Reference};
use sd_file_ext::kind::ObjectKind;
use sd_prisma::{prisma, prisma_sync};
use sd_sync::OperationFactory;
use sd_utils::uuid_to_bytes;
use serde::{Deserialize, Serialize};
use specta::Type;

use serde_json::json;
use uuid::Uuid;

use crate::{
	invalidate_query,
	library::Library,
	object::tag::TagCreateArgs,
	prisma::{file_path, object, tag, tag_on_object},
};

use super::{utils::library, Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("list", {
			R.with2(library()).query(|(_, library), _: ()| async move {
				let tags = library.db.tag().find_many(vec![]).exec().await?;

				let (nodes, items) = tags.normalise(|i| i.id.to_string());

				Ok(NormalisedResults { nodes, items })
			})
		})
		.procedure("getForObject", {
			R.with2(library())
				.query(|(_, library), object_id: i32| async move {
					let tags = library
						.db
						.tag()
						.find_many(vec![tag::tag_objects::some(vec![
							tag_on_object::object_id::equals(object_id),
						])])
						.exec()
						.await?;

					let (nodes, items) = tags.normalise(|i| i.id.to_string());

					Ok(NormalisedResults { nodes, items })
				})
		})
		.procedure("getWithObjects", {
			#[derive(Serialize, Type)]
			pub struct GetWithObjectsResult {
				pub data: BTreeMap<u32, Vec<Reference<tag::Data>>>,
				pub nodes: Vec<CacheNode>,
			}

			#[derive(Serialize, Type)]
			pub struct ObjectWithDateCreated {
				object: Reference<object::Data>,
				date_created: DateTime<Utc>,
			}

			R.with2(library()).query(
				|(_, library), object_ids: Vec<object::id::Type>| async move {
					let Library { db, .. } = library.as_ref();

					let tags_with_objects = db
						.tag()
						.find_many(vec![tag::tag_objects::some(vec![
							tag_on_object::object_id::in_vec(object_ids.clone()),
						])])
						.select(tag::select!({
							id
							tag_objects(vec![tag_on_object::object_id::in_vec(object_ids.clone())]): select {
								date_created
								object: select {
									id
								}
							}
						}))
						.exec()
						.await?;

					// This doesn't need normalised caching because it doesn't return whole models.
					Ok(tags_with_objects
						.into_iter()
						.map(|tag| (tag.id, tag.tag_objects))
						.collect::<BTreeMap<_, _>>())
				},
			)
		})
		.procedure("get", {
			R.with2(library())
				.query(|(_, library), tag_id: i32| async move {
					Ok(library
						.db
						.tag()
						.find_unique(tag::id::equals(tag_id))
						.exec()
						.await?
						.map(|tag| NormalisedResult::from(tag, |i| i.id.to_string())))
				})
		})
		.procedure("create", {
			R.with2(library())
				.mutation(|(_, library), args: TagCreateArgs| async move {
					let created_tag = args.exec(&library).await?;

					invalidate_query!(library, "tags.list");

					Ok(created_tag)
				})
		})
		.procedure("assign", {
			#[derive(Debug, Type, Deserialize)]
			#[specta(inline)]
			enum Target {
				Object(prisma::object::id::Type),
				FilePath(prisma::file_path::id::Type),
			}

			#[derive(Debug, Type, Deserialize)]
			#[specta(inline)]
			struct TagAssignArgs {
				targets: Vec<Target>,
				tag_id: i32,
				unassign: bool,
			}

			R.with2(library())
				.mutation(|(_, library), args: TagAssignArgs| async move {
					let Library { db, sync, .. } = library.as_ref();

					let tag = db
						.tag()
						.find_unique(tag::id::equals(args.tag_id))
						.select(tag::select!({ pub_id }))
						.exec()
						.await?
						.ok_or_else(|| {
							rspc::Error::new(ErrorCode::NotFound, "Tag not found".to_string())
						})?;

					let (objects, file_paths) = db
						._batch({
							let (objects, file_paths): (Vec<_>, Vec<_>) = args
								.targets
								.into_iter()
								.partition_map(|target| match target {
									Target::Object(id) => Either::Left(id),
									Target::FilePath(id) => Either::Right(id),
								});

							(
								db.object()
									.find_many(vec![object::id::in_vec(objects)])
									.select(object::select!({
										id
										pub_id
									})),
								db.file_path()
									.find_many(vec![file_path::id::in_vec(file_paths)])
									.select(file_path::select!({
										id
										pub_id
										object: select { id pub_id }
									})),
							)
						})
						.await?;

					macro_rules! sync_id {
						($pub_id:expr) => {
							prisma_sync::tag_on_object::SyncId {
								tag: prisma_sync::tag::SyncId {
									pub_id: tag.pub_id.clone(),
								},
								object: prisma_sync::object::SyncId { pub_id: $pub_id },
							}
						};
					}

					if args.unassign {
						let query = db.tag_on_object().delete_many(vec![
							tag_on_object::tag_id::equals(args.tag_id),
							tag_on_object::object_id::in_vec(
								objects
									.iter()
									.map(|o| o.id)
									.chain(
										file_paths
											.iter()
											.filter_map(|fp| fp.object.as_ref().map(|o| o.id)),
									)
									.collect(),
							),
						]);

						sync.write_ops(
							db,
							(
								objects
									.into_iter()
									.map(|o| o.pub_id)
									.chain(
										file_paths
											.into_iter()
											.filter_map(|fp| fp.object.map(|o| o.pub_id)),
									)
									.map(|pub_id| sync.relation_delete(sync_id!(pub_id)))
									.collect(),
								query,
							),
						)
						.await?;
					} else {
						let (new_objects, _) = db
							._batch({
								let (left, right): (Vec<_>, Vec<_>) = file_paths
									.iter()
									.filter(|fp| fp.object.is_none())
									.map(|fp| {
										let id = uuid_to_bytes(Uuid::new_v4());

										(
											db.object().create(
												id.clone(),
												vec![
													object::date_created::set(None),
													object::kind::set(Some(
														ObjectKind::Folder as i32,
													)),
												],
											),
											db.file_path().update(
												file_path::id::equals(fp.id),
												vec![file_path::object::connect(
													object::pub_id::equals(id),
												)],
											),
										)
									})
									.unzip();

								(left, right)
							})
							.await?;

						let (sync_ops, db_creates) = objects
							.into_iter()
							.map(|o| (o.id, o.pub_id))
							.chain(
								file_paths
									.into_iter()
									.filter_map(|fp| fp.object.map(|o| (o.id, o.pub_id))),
							)
							.chain(new_objects.into_iter().map(|o| (o.id, o.pub_id)))
							.fold(
								(vec![], vec![]),
								|(mut sync_ops, mut db_creates), (id, pub_id)| {
									db_creates.push(tag_on_object::CreateUnchecked {
										tag_id: args.tag_id,
										object_id: id,
										_params: vec![tag_on_object::date_created::set(Some(
											Utc::now().into(),
										))],
									});

									sync_ops.extend(sync.relation_create(sync_id!(pub_id), []));

									(sync_ops, db_creates)
								},
							);

						sync.write_ops(db, (sync_ops, db.tag_on_object().create_many(db_creates)))
							.await?;
					}

					invalidate_query!(library, "tags.getForObject");
					invalidate_query!(library, "tags.getWithObjects");
					invalidate_query!(library, "search.objects");

					Ok(())
				})
		})
		.procedure("update", {
			#[derive(Type, Deserialize)]
			pub struct TagUpdateArgs {
				pub id: i32,
				pub name: Option<String>,
				pub color: Option<String>,
			}

			R.with2(library())
				.mutation(|(_, library), args: TagUpdateArgs| async move {
					let Library { sync, db, .. } = library.as_ref();

					let tag = db
						.tag()
						.find_unique(tag::id::equals(args.id))
						.select(tag::select!({ pub_id }))
						.exec()
						.await?
						.ok_or(rspc::Error::new(
							ErrorCode::NotFound,
							"Error finding tag in db".into(),
						))?;

					db.tag()
						.update(
							tag::id::equals(args.id),
							vec![tag::date_modified::set(Some(Utc::now().into()))],
						)
						.exec()
						.await?;

					sync.write_ops(
						db,
						(
							[
								args.name.as_ref().map(|v| (tag::name::NAME, json!(v))),
								args.color.as_ref().map(|v| (tag::color::NAME, json!(v))),
							]
							.into_iter()
							.flatten()
							.map(|(k, v)| {
								sync.shared_update(
									prisma_sync::tag::SyncId {
										pub_id: tag.pub_id.clone(),
									},
									k,
									v,
								)
							})
							.collect(),
							db.tag().update(
								tag::id::equals(args.id),
								vec![tag::name::set(args.name), tag::color::set(args.color)],
							),
						),
					)
					.await?;

					invalidate_query!(library, "tags.list");

					Ok(())
				})
		})
		.procedure(
			"delete",
			R.with2(library())
				.mutation(|(_, library), tag_id: i32| async move {
					library
						.db
						.tag()
						.delete(tag::id::equals(tag_id))
						.exec()
						.await?;

					invalidate_query!(library, "tags.list");

					Ok(())
				}),
		)
}
