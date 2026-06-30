/// Minimal linker stubs for TextInputHandler methods.
/// The real implementation is in Rust (text_input_handler.rs).
/// These stubs exist only because C++ JniSupport has a TextInputHandler member.

#include "text_input_handler.h"

void TextInputHandler::enable(std::string, bool) {}
void TextInputHandler::update(std::string) {}
void TextInputHandler::disable() {}
void TextInputHandler::onTextInput(std::string const&) {}
void TextInputHandler::onKeyPressed(KeyCode, KeyAction, int) {}
std::string TextInputHandler::getCopyText() const { return {}; }
void TextInputHandler::setCursorPosition(int) {}
void TextInputHandler::setKeepLastCharOnce() {}
bool TextInputHandler::getKeepLastCharOnce() { return false; }
