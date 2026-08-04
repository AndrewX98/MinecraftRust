#pragma once

#include <game_window.h>

class WindowWithLinuxJoystick : public GameWindow {

public:
    WindowWithLinuxJoystick(std::string const& title, int width, int height, GraphicsApi api);

    ~WindowWithLinuxJoystick() override;

};
