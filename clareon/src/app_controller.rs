// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use clareon_core::ConversationManager;
use std::sync::{Arc, OnceLock};

static RUNTIME: OnceLock<Arc<tokio::runtime::Runtime>> = OnceLock::new();
static MANAGER: OnceLock<Arc<ConversationManager>> = OnceLock::new();

pub fn init_runtime(runtime: Arc<tokio::runtime::Runtime>, manager: Arc<ConversationManager>) {
    RUNTIME.set(runtime).ok();
    MANAGER.set(manager).ok();
}

fn get_runtime() -> Arc<tokio::runtime::Runtime> {
    RUNTIME.get().expect("Runtime not initialized").clone()
}

fn get_manager() -> Arc<ConversationManager> {
    MANAGER.get().expect("Manager not initialized").clone()
}

#[cxx_qt::bridge]
mod app_controller {

    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, current_conversation_id, cxx_name = "currentConversationId")]
        #[qproperty(QString, conversation_title, cxx_name = "conversationTitle")]
        #[qproperty(bool, is_waiting, cxx_name = "isWaiting")]
        #[qproperty(QString, status_message, cxx_name = "statusMessage")]
        #[qproperty(QString, view_mode, cxx_name = "viewMode")]
        #[qproperty(i64, input_tokens, cxx_name = "inputTokens")]
        #[qproperty(i64, output_tokens, cxx_name = "outputTokens")]
        type AppController = super::AppControllerRust;

        #[qsignal]
        #[cxx_name = "conversationChanged"]
        fn conversation_changed(self: Pin<&mut AppController>);

        #[qsignal]
        fn error(self: Pin<&mut AppController>, message: QString);

        #[qsignal]
        #[cxx_name = "conversationsLoaded"]
        fn conversations_loaded(self: Pin<&mut AppController>);

        #[qsignal]
        #[cxx_name = "messagesLoaded"]
        fn messages_loaded(self: Pin<&mut AppController>);

        #[qsignal]
        #[cxx_name = "messageSent"]
        fn message_sent(self: Pin<&mut AppController>);

        #[qsignal]
        #[cxx_name = "streamingUpdate"]
        fn streaming_update(self: Pin<&mut AppController>, text: QString);
    }

    unsafe extern "RustQt" {
        #[qinvokable]
        #[cxx_name = "sendMessage"]
        fn send_message(self: Pin<&mut AppController>, text: &QString);

        #[qinvokable]
        #[cxx_name = "newConversation"]
        fn new_conversation(self: Pin<&mut AppController>);

        #[qinvokable]
        fn search(self: Pin<&mut AppController>, query: &QString);
    }
}

use cxx_qt_lib::QString;
use std::pin::Pin;

pub struct AppControllerRust {
    current_conversation_id: QString,
    conversation_title: QString,
    is_waiting: bool,
    status_message: QString,
    view_mode: QString,
    input_tokens: i64,
    output_tokens: i64,
}

impl Default for AppControllerRust {
    fn default() -> Self {
        Self {
            current_conversation_id: QString::default(),
            conversation_title: QString::from("No conversation"),
            is_waiting: false,
            status_message: QString::from("Ready"),
            view_mode: QString::from("chat"),
            input_tokens: 0,
            output_tokens: 0,
        }
    }
}

impl app_controller::AppController {
    pub fn send_message(mut self: Pin<&mut Self>, text: &QString) {
        use clareon_core::types::ConversationId;

        let message_text = text.to_string();

        if message_text.trim().is_empty() {
            return;
        }

        let conv_id_str = self.current_conversation_id.to_string();
        if conv_id_str.is_empty() {
            self.as_mut()
                .error(QString::from("No conversation selected"));
            return;
        }

        let conv_id = ConversationId::from(conv_id_str);

        self.as_mut().set_is_waiting(true);
        self.as_mut()
            .set_status_message(QString::from("Sending message..."));

        let runtime = get_runtime();
        let manager = get_manager();

        // Use non-streaming send_message for MVP (blocks UI briefly)
        // TODO: Implement streaming once we figure out cxx-qt 0.8 threading pattern
        let result = runtime.block_on(async {
            let mut conversation = manager.load_conversation(&conv_id).await?;
            manager.send_message(&mut conversation, &message_text).await
        });

        match result {
            Ok(response) => {
                self.as_mut().set_input_tokens(response.usage.input_tokens);
                self.as_mut()
                    .set_output_tokens(response.usage.output_tokens);
                self.as_mut().set_is_waiting(false);
                self.as_mut().set_status_message(QString::from("Ready"));
                self.as_mut().message_sent();
            }
            Err(e) => {
                self.as_mut().error(QString::from(&format!("{}", e)));
                self.as_mut().set_is_waiting(false);
                self.as_mut().set_status_message(QString::from("Error"));
            }
        }
    }

    pub fn new_conversation(mut self: Pin<&mut Self>) {
        let runtime = get_runtime();
        let manager = get_manager();

        let result = runtime.block_on(async { manager.new_conversation().await });
        let conversation = match result {
            Ok(conv) => conv,
            Err(e) => {
                self.as_mut().error(QString::from(&format!("{}", e)));
                return;
            }
        };

        self.as_mut()
            .set_current_conversation_id(conversation.id.to_string().into());
        self.as_mut()
            .set_conversation_title(conversation.display_title().into());
        self.as_mut()
            .set_status_message(QString::from("Started new conversation"));
        self.as_mut().conversation_changed();
    }

    pub fn search(mut self: Pin<&mut Self>, query: &QString) {
        let query_text = query.to_string();

        if query_text.trim().is_empty() {
            self.as_mut().set_view_mode(QString::from("chat"));
            return;
        }

        self.as_mut().set_view_mode(QString::from("search"));
        self.as_mut().set_status_message(QString::from(
            format!("Searching for '{}'...", query_text).as_str(),
        ));
    }
}
