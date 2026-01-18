[Desktop Entry]
GenericName=AI Assistant
Name=Clareon
Comment=Chat with Anthropic models from native Linux app
Categories=Qt;KDE;Utility;
Keywords=ai,assistant,anthropic,claude,chat
Exec=${CMAKE_INSTALL_PREFIX}/${CMAKE_INSTALL_BINDIR}/clareon %U
Icon=clareon
Type=Application
StartupNotify=false
Actions=NewConversation
DBusActivatable=false
X-KDE-Shortcuts=Meta+C

[Desktop Action NewConversation]
Name=New Conversation
Exec=${CMAKE_INSTALL_PREFIX}/${CMAKE_INSTALL_BINDIR}/clareon --new-conversation
