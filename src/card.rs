use actix_web::web::{self, Data};
use actix_web::{delete, get, post, put, Result as AwResult};
use maud::Markup;
use serde::Deserialize;

use crate::board::board_data;
use crate::db::QueryId;
use crate::util::{CustomError, Helper, ParseIndexVector, RemoveCard, ToJson};
use crate::{db, html, models, AppState};

#[get("/{id}")]
async fn get(state: Data<AppState>, path: web::Path<i64>) -> AwResult<Markup> {
    let id = path.into_inner();
    let card = db::Card::query_id(id, &state.db).await?;
    Ok(html::make_card(card))
}

#[get("/edit/{id}")]
async fn edit_get(state: Data<AppState>, path: web::Path<i64>) -> AwResult<Markup> {
    let id = path.into_inner();
    let card = db::Card::query_id(id, &state.db).await?;
    Ok(html::edit_card(card))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct EditCard {
    card_id: i64,
    title: String,
}

#[put("/edit/{id}")]
async fn edit_put(state: Data<AppState>, web::Form(form): web::Form<EditCard>) -> AwResult<Markup> {
    let EditCard { card_id, title } = form;
    let title = title.trim();

    sqlx::query!("UPDATE cards SET title = ? WHERE id = ?", title, card_id)
        .execute(&state.db)
        .await
        .ensure_query_success()?;

    let card = db::Card::query_id(card_id, &state.db).await?;
    Ok(html::make_card(card))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct MoveCard {
    card_id: i64,
    to_list_id: i64,
    new_position: i64,
}

#[post("/move")]
async fn move_(state: Data<AppState>, web::Form(form): web::Form<MoveCard>) -> AwResult<Markup> {
    let MoveCard {
        card_id,
        to_list_id,
        new_position,
    } = form;

    #[derive(Debug)]
    struct Query {
        id: i64,
        cards_order: String,
    }

    let mut query: Vec<Query> = sqlx::query_as!(
        Query,
        "SELECT id, cards_order FROM lists WHERE id IN
        (?, (SELECT list_id FROM cards WHERE id = ?));",
        to_list_id,
        card_id,
    )
    .fetch_all(&state.db)
    .await
    .ensure_query_success()?;

    let query_len = query.len();
    if !(query_len == 1 || query_len == 2) {
        dbg!(&query);
        return Err(CustomError::InsufficientItemsReturned(format!(
            "Move card query not 1 or 2 ({query_len}): to list id: {to_list_id}, card id {card_id}"
        ))
        .into());
    }
    let actual_position = |new_position: i64, len: usize| -> usize {
        if new_position < 0 {
            // Not found, insert at the end
            len
        } else {
            // Don't insert after len
            (new_position as usize).min(len)
        }
    };

    if query_len == 1 {
        let list = query.pop().unwrap();

        let mut cards_order = list
            .cards_order
            .parse_index_vector()?
            .remove_card(card_id)?;
        cards_order.insert(actual_position(new_position, cards_order.len()), card_id);
        let cards_order = cards_order.to_json();

        sqlx::query!(
            "BEGIN TRANSACTION;
            UPDATE lists SET cards_order = ? WHERE id = ?;
            UPDATE cards SET list_id = ? WHERE id = ?;
            COMMIT;",
            cards_order,
            list.id,
            list.id,
            card_id,
        )
        .execute(&state.db)
        .await
        .ensure_query_success()?;
    } else {
        let popped = query.pop().unwrap();

        let (to_list, from_list) = if popped.id == to_list_id {
            (popped, query.pop().unwrap())
        } else {
            (query.pop().unwrap(), popped)
        };

        let from_cards_order = from_list
            .cards_order
            .parse_index_vector()?
            .remove_card(card_id)?
            .to_json();

        let mut to_cards_order = to_list.cards_order.parse_index_vector()?;
        to_cards_order.insert(actual_position(new_position, to_cards_order.len()), card_id);
        let to_cards_order = to_cards_order.to_json();

        sqlx::query!(
            "BEGIN TRANSACTION;
            UPDATE lists SET cards_order = ? WHERE id = ?;
            UPDATE lists SET cards_order = ? WHERE id = ?;
            UPDATE cards SET list_id = ? WHERE id = ?;
            COMMIT;",
            from_cards_order,
            from_list.id,
            to_cards_order,
            to_list.id,
            to_list.id,
            card_id,
        )
        .execute(&state.db)
        .await
        .ensure_query_success()?;
    }

    let board = board_data(state).await?.1;

    Ok(html::make_board(board))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct DeleteCard {
    card_id: i64,
}

#[delete("")]
async fn delete(state: Data<AppState>, web::Form(form): web::Form<DeleteCard>) -> AwResult<Markup> {
    let DeleteCard { card_id } = form;

    #[derive(Debug)]
    struct Query {
        id: i64,
        cards_order: String,
    }

    let query: Option<Query> = sqlx::query_as!(
        Query,
        "SELECT id, cards_order FROM lists WHERE id IN (SELECT list_id FROM cards WHERE id = ?);",
        form.card_id,
    )
    .fetch_optional(&state.db)
    .await
    .ensure_query_success()?;

    let Some(query) = query else {
        return Err(CustomError::InsufficientItemsReturned(format!(
            "Could not find list associated with {}",
            card_id
        ))
        .into());
    };

    let cards_order = query
        .cards_order
        .parse_index_vector()?
        .remove_card(card_id)?
        .to_json();

    sqlx::query!(
        "BEGIN TRANSACTION;
        UPDATE lists SET cards_order = ? WHERE id = ?;
        DELETE FROM cards WHERE id = ?;
        COMMIT;",
        cards_order,
        query.id,
        card_id,
    )
    .execute(&state.db)
    .await
    .ensure_query_success()?;

    Ok(html::make_board(board_data(state).await?.1))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct NewCard {
    list_id: i64,
    title: String,
}

#[post("")]
async fn post_new(state: Data<AppState>, web::Form(form): web::Form<NewCard>) -> AwResult<Markup> {
    let NewCard { list_id, title } = form;

    sqlx::query!(
        "BEGIN TRANSACTION;
        INSERT INTO cards (list_id, title) VALUES (?, ?);
        UPDATE lists
            SET cards_order = json_insert(cards_order, '$[#]', last_insert_rowid())
            WHERE id = ?;
        COMMIT;",
        list_id,
        title,
        list_id,
    )
    .execute(&state.db)
    .await
    .ensure_query_success()?;

    let mut list = models::ListData::query_id(list_id, &state.db).await?;

    let cards: Vec<db::Card> =
        sqlx::query_as!(db::Card, "SELECT * FROM cards WHERE list_id = ?", list_id)
            .fetch_all(&state.db)
            .await
            .ensure_data_type()?;

    list.cards = list
        .cards_order
        .iter()
        .filter_map(|&idx| cards.iter().find(|&card| card.id == idx))
        .cloned()
        .collect();

    Ok(html::make_list(list.into()))
}

pub fn service() -> actix_web::Scope {
    web::scope("/card")
        .service(move_)
        .service(delete)
        .service(get)
        .service(post_new)
        .service(edit_get)
        .service(edit_put)
}
