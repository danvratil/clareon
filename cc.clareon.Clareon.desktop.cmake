[Desktop Entry]
GenericName=AI Assistant
Name=Clareon
Comment=Chat with Anthropic models from native Linux app
Categories=Qt;KDE;Utility;
Keywords=ai,assistant,anthropic,openai,chatgpt,chat
Exec=clareon %U
Icon=cc.clareon.Clareon
Type=Application
StartupNotify=false
Actions=QuickInput
DBusActivatable=false

[Desktop Action QuickInput]
Name=Quick Input
Exec=${CMAKE_INSTALL_PREFIX}/${CMAKE_INSTALL_BINDIR}/clareon --quick-input
X-KDE-Shortcuts=Meta+C
