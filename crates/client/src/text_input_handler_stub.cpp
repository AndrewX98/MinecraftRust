/// Stub replacing text_input_handler.cpp for the Rust build.
/// Delegates TextInputHandler logic to Rust via extern "C" functions.

#include "text_input_handler.h"
#include "utf8_util.h"

// --- Rust functions ---
extern "C" void text_input_handler_enable(TextInputHandler *self, const char *text, bool multiline);
extern "C" void text_input_handler_update(TextInputHandler *self, const char *text);
extern "C" void text_input_handler_disable(TextInputHandler *self);
extern "C" void text_input_handler_on_text_input(TextInputHandler *self, const char *text);
extern "C" void text_input_handler_on_key_pressed(TextInputHandler *self, int key, int action, int mods);
extern "C" const char *text_input_handler_get_copy_text(TextInputHandler *self, size_t *len);
extern "C" void text_input_handler_set_cursor_position(TextInputHandler *self, int pos);
extern "C" void text_input_handler_set_keep_last_char_once(TextInputHandler *self);
extern "C" bool text_input_handler_get_keep_last_char_once(TextInputHandler *self);

// Keep the C++ implementation for now - the class is tightly coupled with std::function
// callbacks and small UTF-8 text processing logic. Hybris hooks are registered from Rust.

void TextInputHandler::enable(std::string text, bool multiline_) {
    enabled = true;
    this->multiline = multiline_;
    enabledNo++;
    if(keepOnce) {
        printf("[TextInputHandler::update] %s\n", currentText.data());
        printf("[TextInputHandler::update] %s\n", text.data());
        if(currentText.length()) {
            auto size = currentText.size();
            if(size >= 3 && UTF8Util::getCharByteSize(currentText[currentText.size() - 3]) == 3) {
                text += currentText.substr(currentText.length() - 3);
            } else if(size >= 2 && UTF8Util::getCharByteSize(currentText[currentText.size() - 2]) == 2) {
                text += currentText.substr(currentText.length() - 2);
            } else if(size >= 1 && UTF8Util::getCharByteSize(currentText[currentText.size() - 1]) == 1) {
                text += currentText[currentText.length() - 1];
            }
            textUpdateCallback(text);
        }
        keepOnce = false;
    }
    update(std::move(text));
}

void TextInputHandler::update(std::string text) {
    currentText = std::move(text);
    currentTextPosition = currentText.size();
    currentTextPositionUTF = UTF8Util::getLength(currentText.c_str(), currentTextPosition);
    currentTextCopyPosition = currentTextPosition;
    currentTextCopyPositionUTF = currentTextPositionUTF;
}

void TextInputHandler::disable() {
    if(!keepOnce) {
        currentText.clear();
        currentTextPosition = 0;
        currentTextPositionUTF = 0;
        currentTextCopyPosition = 0;
        currentTextCopyPositionUTF = 0;
        enabled = false;
    }
}

void TextInputHandler::onTextInput(std::string const& val) {
    if(!enabled) {
        textUpdateCallback(val);
        return;
    }
    if(val.size() == 1 && val[0] == 8) {
        if(currentTextPositionUTF <= 0)
            return;
        auto deleteStart = currentTextPosition - 1;
        if(altPressed) {
            if(strchr(spaces, currentText[deleteStart])) {
                while(deleteStart > 0) {
                    deleteStart--;
                    if(deleteStart < 1 || !strchr(spaces, currentText[deleteStart - 1]) || !strchr(spaces, currentText[deleteStart])) {
                        break;
                    }
                }
            }
            while(deleteStart > 0) {
                deleteStart--;
                if(deleteStart < 1 || strchr(spaces, currentText[deleteStart - 1]) || strchr(spaces, currentText[deleteStart])) {
                    break;
                }
            }
            currentTextPositionUTF = deleteStart;
        } else {
            currentTextPositionUTF--;
        }
        while(deleteStart > 0 && (currentText[deleteStart] & 0b11000000) == 0b10000000)
            deleteStart--;
        currentText.erase(currentText.begin() + deleteStart, currentText.begin() + currentTextPosition);
        currentTextPosition = deleteStart;
    } else if(val.size() == 1 && val[0] == 127) {
        if(currentTextPosition >= currentText.size())
            return;
        auto deleteEnd = currentTextPosition + 1;
        while(deleteEnd < currentText.size() && (currentText[deleteEnd] & 0b11000000) == 0b10000000)
            deleteEnd++;
        currentText.erase(currentText.begin() + currentTextPosition, currentText.begin() + deleteEnd);
    } else {
        currentText.insert(currentText.begin() + currentTextPosition, val.begin(), val.end());
        currentTextPosition += val.size();
        currentTextPositionUTF += UTF8Util::getLength(val.c_str(), val.size());
    }
    textUpdateCallback(currentText);
    currentTextCopyPosition = currentTextPosition;
    currentTextCopyPositionUTF = currentTextPositionUTF;
    caretPositionCallback(getCursorPosition());
}

void TextInputHandler::onKeyPressed(KeyCode key, KeyAction action, int mods) {
    shiftPressed = mods & KEY_MOD_SHIFT;
    altPressed = mods & KEY_MOD_ALT;

    if(action != KeyAction::PRESS && action != KeyAction::REPEAT)
        return;
    if(key == KeyCode::RIGHT) {
        if(!shiftPressed && currentTextPosition != currentTextCopyPosition) {
            currentTextPosition = std::max(currentTextPosition, currentTextCopyPosition);
            currentTextPositionUTF = UTF8Util::getLength(currentText.c_str(), currentTextPosition);
            currentTextCopyPosition = currentTextPosition;
            currentTextCopyPositionUTF = currentTextPositionUTF;
            caretPositionCallback(getCursorPosition());
            return;
        }
        if(currentTextPosition >= currentText.size())
            return;
        if(altPressed) {
            if(strchr(spaces, currentText[currentTextPosition])) {
                while(currentTextPosition < currentText.size()) {
                    currentTextPosition++;
                    if(currentTextPosition >= currentText.size() || !strchr(spaces, currentText[currentTextPosition])) {
                        break;
                    }
                }
            }
            while(currentTextPosition < currentText.size()) {
                currentTextPosition++;
                if(currentTextPosition >= currentText.size() || strchr(spaces, currentText[currentTextPosition])) {
                    break;
                }
            }
            currentTextPositionUTF = UTF8Util::getLength(currentText.c_str(), currentTextPosition);
        } else {
            currentTextPosition++;
            while(currentTextPosition < currentText.size() &&
                  (currentText[currentTextPosition] & 0b11000000) == 0b10000000)
                currentTextPosition++;
            currentTextPositionUTF++;
        }
    } else if(key == KeyCode::LEFT) {
        if(!shiftPressed && currentTextPosition != currentTextCopyPosition) {
            currentTextPosition = std::min(currentTextPosition, currentTextCopyPosition);
            currentTextPositionUTF = UTF8Util::getLength(currentText.c_str(), currentTextPosition);
            currentTextCopyPosition = currentTextPosition;
            currentTextCopyPositionUTF = currentTextPositionUTF;
            caretPositionCallback(getCursorPosition());
            return;
        }
        if(currentTextPosition <= 0)
            return;
        if(altPressed) {
            if(strchr(spaces, currentText[currentTextPosition - 1])) {
                while(currentTextPosition > 0) {
                    currentTextPosition--;
                    if(currentTextPosition < 1 || !strchr(spaces, currentText[currentTextPosition - 1])) {
                        break;
                    }
                }
            }
            while(currentTextPosition > 0) {
                currentTextPosition--;
                if(currentTextPosition < 1 || strchr(spaces, currentText[currentTextPosition - 1])) {
                    break;
                }
            }
            currentTextPositionUTF = UTF8Util::getLength(currentText.c_str(), currentTextPosition);
        } else {
            currentTextPosition--;
            while(currentTextPosition > 0 && (currentText[currentTextPosition] & 0b11000000) == 0b10000000)
                currentTextPosition--;
            currentTextPositionUTF--;
        }
    } else if(key == KeyCode::HOME) {
        currentTextPosition = 0;
        currentTextPositionUTF = 0;
    } else if(key == KeyCode::END) {
        currentTextPosition = currentText.size();
        currentTextPositionUTF = UTF8Util::getLength(currentText.c_str(), currentTextPosition);
    } else {
        return;
    }
    if(!shiftPressed) {
        currentTextCopyPosition = currentTextPosition;
        currentTextCopyPositionUTF = currentTextPositionUTF;
    }
    caretPositionCallback(getCursorPosition());
}

std::string TextInputHandler::getCopyText() const {
    if(currentTextCopyPosition != currentTextPosition) {
        size_t start = std::min(currentTextPosition, currentTextCopyPosition);
        size_t end = std::max(currentTextPosition, currentTextCopyPosition);
        return currentText.substr(start, end - start);
    } else {
        return currentText;
    }
}

void TextInputHandler::setCursorPosition(int pos) {
    if(pos < 0) {
        currentTextPosition = currentText.size();
        currentTextPositionUTF = UTF8Util::getLength(currentText.c_str(), currentTextPosition);
    } else {
        currentTextPositionUTF = pos;
        currentTextPosition = UTF8Util::getBytePosFromUTF(currentText.c_str(), currentText.size(), pos);
    }
    if(!shiftPressed) {
        currentTextCopyPosition = currentTextPosition;
        currentTextCopyPositionUTF = currentTextPositionUTF;
    }
    caretPositionCallback(getCursorPosition());
}

void TextInputHandler::setKeepLastCharOnce() {
    keepOnce = true;
}

bool TextInputHandler::getKeepLastCharOnce() {
    return keepOnce;
}
