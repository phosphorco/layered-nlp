#![doc(
    html_logo_url = "https://raw.githubusercontent.com/storyscript/layered-nlp/main/assets/layered-nlp.svg",
    issue_tracker_base_url = "https://github.com/storyscript/layered-nlp/issues/"
)]

mod clause_keyword;
mod clause_link;
mod clause_link_resolver;
mod clause_query;
mod clauses;

pub use clause_keyword::{ClauseKeyword, ClauseKeywordResolver};
pub use clause_link::ClauseLinkBuilder;
pub use clause_link_resolver::{ClauseLink, ClauseLinkResolver, ClauseSpan};
pub use clause_query::ClauseQueryAPI;
pub use clauses::{Clause, ClauseResolver};

#[cfg(test)]
mod tests {
    mod clauses;
    mod integration;
}
