use dioxus::prelude::*;

#[hotpath::main]
fn main() {
    dioxus::launch(App);
}

#[component]
#[allow(non_snake_case)]
fn App() -> Element {
    rsx! {
        main {
            h1 { "WF Observer" }
            p { "Dioxus client showcase" }
        }
    }
}
