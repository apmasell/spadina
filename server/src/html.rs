use horrorshow::prelude::*;

#[cfg(feature = "wasm-client")]
pub fn create_main() -> String {
  let contents = horrorshow::html! {
       : horrorshow::helper::doctype::HTML;
       head {
           title: "Puzzleverse";
           css {
              : Raw("html, body { margin: 0 !important; padding: 0 !important; } canvas { position:fixed; left:0; top:0; width:100%; height:100%; }")
           }
           script(type="module"){
              : "import init from './puzzleverse-client.js'; init();"
           }
       }
       body {
           canvas(id="puzzleverse");
       }
  };
  contents.into_string().unwrap()
}

#[cfg(not(feature = "wasm-client"))]
pub fn create_main() -> String {
  let contents = horrorshow::html! {
       : horrorshow::helper::doctype::HTML;
       head {
           title: "Puzzleverse";
       }
       body {
           p {
               : "Please download a Puzzleverse client and connect to this server.";
           }
       }
  };
  contents.into_string().unwrap()
}
pub fn create_oauth_result(message: &str) -> String {
  let contents = horrorshow::html! {
       : horrorshow::helper::doctype::HTML;
       head {
           title: "Puzzleverse";
           css {
              : Raw("html, body { margin: 0 !important; padding: 0 !important; }")
           }
       }
       body {
            p {
                :message
            }
       }
  };
  contents.into_string().unwrap()
}

pub(crate) fn create_oauth_register<T: serde::Serialize>(
  invitation: Option<impl AsRef<str>>,
  registration: impl Iterator<Item = (String, T)>,
  error_message: Option<&str>,
) -> String {
  let error_message = error_message.map(|message| {
    horrorshow::owned_html! {
        p(style="text-color: red") {
            p {
              : message
            }
          }
    }
  });
  let invitation = invitation.map(|invitation| {
    horrorshow::owned_html! {
        p {

          label {
              : "Invitation:"
          }
            input(type="hidden", id="invitation", value=invitation.as_ref());
        }
    }
  });
  let contents = horrorshow::html! {
       : horrorshow::helper::doctype::HTML;
       head {
           title: "Puzzleverse";
       }
       body {
           form(method="POST", action="register-next") {
              : error_message;
              label {
                  : "Player:"
              }
              input(type="hidden", id="player");
              br;
              : invitation;
              @ for (text, value) in registration {
                  input(type="submit", name="client_type", value=serde_json::to_string(&value).expect("Failed to serialize value from OpenID provider.")) {
                      :text
                  }
                  br;
              }
          }
       }
  };
  contents.into_string().unwrap()
}
