pub mod view {
    use maud::{Markup, html};

    pub fn head() -> Markup {
        html! {
            head {
                title { "Docker Registry Explorer" }
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                link rel="stylesheet" href="/static/css/main.css";
            }
        }
    }
}
