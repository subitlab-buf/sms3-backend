use reqwest::{RequestBuilder, Response};

pub struct CacheImg<'a> {
    pub account_info: &'a crate::AccoutInfo,
    pub file: std::sync::Mutex<Option<tokio::fs::File>>,
}

#[async_trait::async_trait]
impl super::Request for CacheImg<'_> {
    type Output = u64;
    const URL_SUFFIX: &'static str = "/api/post/upload-image";

    fn make_req(&self, req: RequestBuilder) -> anyhow::Result<RequestBuilder> {
        Ok(req.headers(self.account_info.into()).body(
            self.file
                .lock()
                .unwrap()
                .take()
                .ok_or_else(|| anyhow::anyhow!("file should not be none"))?,
        ))
    }

    async fn parse_res(&mut self, response: Response) -> anyhow::Result<Self::Output> {
        #[derive(serde::Deserialize)]
        struct Res {
            hash: u64,
        }

        Ok(response.json::<Res>().await?.hash)
    }
}

pub struct GetImg<'a> {
    pub account_info: &'a crate::AccoutInfo,
    pub hash: u64,
}

#[async_trait::async_trait]
impl super::Request for GetImg<'_> {
    type Output = image::DynamicImage;
    const URL_SUFFIX: &'static str = "/api/post/get-image";

    fn make_req(&self, req: RequestBuilder) -> anyhow::Result<RequestBuilder> {
        Ok(req
            .headers(self.account_info.into())
            .json(&sms3_shared::post::handle::GetImageDescriptor { hash: self.hash }))
    }

    async fn parse_res(&mut self, response: Response) -> anyhow::Result<Self::Output> {
        Ok(image::load_from_memory(&response.bytes().await?)?)
    }
}

pub struct New<'a> {
    pub account_info: &'a crate::AccoutInfo,
    pub title: String,
    pub description: String,
    pub time_range: std::ops::RangeInclusive<chrono::NaiveDate>,
    pub images: Vec<u64>,
}

#[async_trait::async_trait]
impl super::Request for New<'_> {
    type Output = ();
    const URL_SUFFIX: &'static str = "/api/post/create";

    fn make_req(&self, req: RequestBuilder) -> anyhow::Result<RequestBuilder> {
        Ok(req
            .headers(self.account_info.into())
            .json(&sms3_shared::post::handle::PostDescriptor {
                title: self.title.to_owned(),
                description: self.description.to_owned(),
                time_range: (*self.time_range.start(), *self.time_range.end()),
                images: self.images.iter().copied().collect(),
            }))
    }

    async fn parse_res(&mut self, _response: Response) -> anyhow::Result<Self::Output> {
        Ok(())
    }
}

pub struct Get<'a> {
    pub account_info: &'a crate::AccoutInfo,
    pub filters: &'a [sms3_shared::post::handle::GetPostsFilter],
}

#[async_trait::async_trait]
impl super::Request for Get<'_> {
    type Output = Vec<u64>;
    const URL_SUFFIX: &'static str = "/api/post/get";

    fn make_req(&self, req: RequestBuilder) -> anyhow::Result<RequestBuilder> {
        Ok(req.headers(self.account_info.into()).json(
            &sms3_shared::post::handle::GetPostsDescriptor {
                filters: self.filters.to_vec(),
            },
        ))
    }

    async fn parse_res(&mut self, response: Response) -> anyhow::Result<Self::Output> {
        #[derive(serde::Deserialize)]
        struct Res {
            posts: Vec<u64>,
        }

        Ok(response.json::<Res>().await?.posts)
    }
}

pub struct GetInfos<'a> {
    pub account_info: &'a crate::AccoutInfo,
    pub map: &'a mut std::collections::HashMap<u64, Option<anyhow::Result<crate::Post>>>,
}

#[async_trait::async_trait]
impl super::Request for GetInfos<'_> {
    type Output = ();
    const URL_SUFFIX: &'static str = "/api/post/get-info";

    fn make_req(&self, req: RequestBuilder) -> anyhow::Result<RequestBuilder> {
        Ok(req.headers(self.account_info.into()).json(
            &sms3_shared::post::handle::GetPostsInfoDescriptor {
                posts: self.map.keys().into_iter().copied().collect(),
            },
        ))
    }

    async fn parse_res(&mut self, response: Response) -> anyhow::Result<Self::Output> {
        #[derive(serde::Deserialize)]
        struct Res {
            results: Vec<sms3_shared::post::handle::GetPostInfoResult>,
        }

        for result in response.json::<Res>().await?.results {
            match result {
                sms3_shared::post::handle::GetPostInfoResult::Full(post) => {
                    self.map.insert(
                        post.id,
                        Some(Ok(crate::Post {
                            images: post.images,
                            title: post.metadata.title,
                            archived: post.metadata.time_range.1 < chrono::Utc::now().date_naive(),
                            ext: Some(crate::PostExt {
                                description: post.metadata.description,
                                time: (post.metadata.time_range.0..=post.metadata.time_range.1),
                                publisher: crate::LazyUser::new(post.publisher),
                                status: post
                                    .status
                                    .into_iter()
                                    .map(|value| crate::PostAccept {
                                        operator: crate::LazyUser::new(value.operator),
                                        status: value.status,
                                        time: value.time,
                                    })
                                    .collect(),
                            }),
                        })),
                    );
                }
                sms3_shared::post::handle::GetPostInfoResult::Foreign {
                    id,
                    images,
                    title,
                    archived,
                } => {
                    self.map.insert(
                        id,
                        Some(Ok(crate::Post {
                            images,
                            title,
                            archived,
                            ext: None,
                        })),
                    );
                }
                sms3_shared::post::handle::GetPostInfoResult::NotFound(id) => {
                    self.map
                        .insert(id, Some(Err(anyhow::anyhow!("post not found"))));
                }
            }
        }

        Ok(())
    }
}

pub struct Edit<'a> {
    pub account_info: &'a crate::AccoutInfo,
    pub post: u64,
    pub actions: &'a [sms3_shared::post::handle::EditPostVariant],
}

#[async_trait::async_trait]
impl super::Request for Edit<'_> {
    type Output = ();
    const URL_SUFFIX: &'static str = "/api/post/edit";

    fn make_req(&self, req: RequestBuilder) -> anyhow::Result<RequestBuilder> {
        Ok(req.headers(self.account_info.into()).json(
            &sms3_shared::post::handle::EditPostDescriptor {
                post: self.post,
                variants: self.actions.to_vec(),
            },
        ))
    }

    async fn parse_res(&mut self, _response: Response) -> anyhow::Result<Self::Output> {
        Ok(())
    }
}

pub struct Approve<'a> {
    pub account_info: &'a crate::AccoutInfo,
    pub post: u64,
    pub action: sms3_shared::post::handle::ApprovePostVariant,
}

#[async_trait::async_trait]
impl super::Request for Approve<'_> {
    type Output = ();
    const URL_SUFFIX: &'static str = "/api/post/approve";

    fn make_req(&self, req: RequestBuilder) -> anyhow::Result<RequestBuilder> {
        Ok(req.headers(self.account_info.into()).json(
            &sms3_shared::post::handle::ApprovePostDescriptor {
                post: self.post,
                variant: self.action.clone(),
            },
        ))
    }

    async fn parse_res(&mut self, _response: Response) -> anyhow::Result<Self::Output> {
        Ok(())
    }
}
