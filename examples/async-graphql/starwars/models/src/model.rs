#![allow(clippy::needless_lifetimes)]

use crate::{StarWars, StarWarsChar};
use async_graphql::connection::{query, Connection, Edge};
use async_graphql::{Context, Enum, Interface, Object, OutputType, Result};
use std::fmt::Error;

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum Episode {
    NewHope,
    Empire,
    Jedi,
}

pub struct Human<'a>(&'a StarWarsChar);

#[Object]
impl<'a> Human<'a> {
    async fn id(&self) -> &str {
        self.0.id
    }

    async fn name(&self) -> &str {
        self.0.name
    }

    async fn friends<'ctx>(&self, ctx: &Context<'ctx>) -> Vec<Character<'ctx>> {
        let star_wars = ctx.data_unchecked::<StarWars>();
        star_wars
            .friends(self.0)
            .into_iter()
            .map(|ch| {
                if ch.is_human {
                    Human(ch).into()
                } else {
                    Droid(ch).into()
                }
            })
            .collect()
    }

    async fn appears_in(&self) -> &[Episode] {
        &self.0.appears_in
    }

    async fn home_planet(&self) -> &Option<&str> {
        &self.0.home_planet
    }
}

pub struct Droid<'a>(&'a StarWarsChar);

#[Object]
impl<'a> Droid<'a> {
    async fn id(&self) -> &str {
        self.0.id
    }

    async fn name(&self) -> &str {
        self.0.name
    }

    async fn friends<'ctx>(&self, ctx: &Context<'ctx>) -> Vec<Character<'ctx>> {
        let star_wars = ctx.data_unchecked::<StarWars>();
        star_wars
            .friends(self.0)
            .into_iter()
            .map(|ch| {
                if ch.is_human {
                    Human(ch).into()
                } else {
                    Droid(ch).into()
                }
            })
            .collect()
    }

    async fn appears_in(&self) -> &[Episode] {
        &self.0.appears_in
    }

    async fn primary_function(&self) -> &Option<&str> {
        &self.0.primary_function
    }
}

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn hero<'a>(
        &self,
        ctx: &Context<'a>,
        #[graphql(
            desc = "If omitted, returns the hero of the whole saga. If provided, returns the hero of that particular episode."
        )]
        episode: Option<Episode>,
    ) -> Character<'a> {
        let star_wars = ctx.data_unchecked::<StarWars>();
        match episode {
            Some(episode) => {
                if episode == Episode::Empire {
                    Human(star_wars.chars.get(star_wars.luke).unwrap()).into()
                } else {
                    Droid(star_wars.chars.get(star_wars.artoo).unwrap()).into()
                }
            }
            None => Human(star_wars.chars.get(star_wars.luke).unwrap()).into(),
        }
    }

    async fn human<'a>(
        &self,
        ctx: &Context<'a>,
        #[graphql(desc = "id of the human")] id: String,
    ) -> Option<Human<'a>> {
        ctx.data_unchecked::<StarWars>().human(&id).map(Human)
    }

    async fn humans<'a>(
        &self,
        ctx: &Context<'a>,
        after: Option<String>,
        before: Option<String>,
        first: Option<i32>,
        last: Option<i32>,
    ) -> Result<Connection<usize, Human<'a>>> {
        let humans = ctx.data_unchecked::<StarWars>().humans().to_vec();
        query_characters(after, before, first, last, &humans, Human).await
    }

    async fn droid<'a>(
        &self,
        ctx: &Context<'a>,
        #[graphql(desc = "id of the droid")] id: String,
    ) -> Option<Droid<'a>> {
        ctx.data_unchecked::<StarWars>().droid(&id).map(Droid)
    }

    async fn droids<'a>(
        &self,
        ctx: &Context<'a>,
        after: Option<String>,
        before: Option<String>,
        first: Option<i32>,
        last: Option<i32>,
    ) -> Result<Connection<usize, Droid<'a>>> {
        let droids = ctx.data_unchecked::<StarWars>().droids().to_vec();
        query_characters(after, before, first, last, &droids, Droid).await
    }
}

#[derive(Interface)]
#[graphql(
    field(name = "id", ty = "&str"),
    field(name = "name", ty = "&str"),
    field(name = "friends", ty = "Vec<Character<'ctx>>"),
    field(name = "appears_in", ty = "&[Episode]")
)]
pub enum Character<'a> {
    Human(Human<'a>),
    Droid(Droid<'a>),
}

async fn query_characters<'a, F, T>(
    after: Option<String>,
    before: Option<String>,
    first: Option<i32>,
    last: Option<i32>,
    characters: &[&'a StarWarsChar],
    map_to: F,
) -> Result<Connection<usize, T>>
where
    F: Fn(&'a StarWarsChar) -> T,
    T: OutputType,
{
    query(
        after,
        before,
        first,
        last,
        |after, before, first, last| async move {
            let mut start = 0usize;
            let mut end = characters.len();

            if let Some(after) = after {
                if after >= characters.len() {
                    return Ok(Connection::new(false, false));
                }
                start = after + 1;
            }

            if let Some(before) = before {
                if before == 0 {
                    return Ok(Connection::new(false, false));
                }
                end = before;
            }

            let mut slice = &characters[start..end];

            if let Some(first) = first {
                slice = &slice[..first.min(slice.len())];
                end -= first.min(slice.len());
            } else if let Some(last) = last {
                slice = &slice[slice.len() - last.min(slice.len())..];
                start = end - last.min(slice.len());
            }

            let mut connection = Connection::new(start > 0, end < characters.len());
            connection.edges.extend(
                slice
                    .iter()
                    .enumerate()
                    .map(|(idx, item)| Edge::new(start + idx, (map_to)(item))),
            );
            Ok::<_, Error>(connection)
        },
    )
    .await
}
