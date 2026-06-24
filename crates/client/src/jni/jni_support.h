#pragma once

#include "main_activity.h"
#include "store.h"
#include "fake_assetmanager.h"
#include <baron/baron.h>
#include <android/native_activity.h>
#include <android/game_activity.h>
#include <game_window.h>
#include <condition_variable>
#include <mutex>
#include "text_input_handler.h"

struct JniSupport {
private:
    struct NativeEntry {
        const char *name;
        const char *sig;
    };

    bool isGameActivity;

    Baron::Jvm vm;
    ANativeActivityCallbacks nativeActivityCallbacks;
    GameActivityCallbacks gameActivityCallbacks;
    ANativeActivity nativeActivity;
    GameActivity gameActivity;
    std::shared_ptr<MainActivity> activity;
    jobject activityRef;
    std::unique_ptr<FakeAssetManager> assetManager;
    ANativeWindow *window;
    AInputQueue *inputQueue;
    std::condition_variable gameExitCond;
    std::mutex gameExitMutex;
    bool gameExitVal = false, looperRunning = false;
    TextInputHandler textInput;

    void registerJniClasses();

    void registerNatives(std::shared_ptr<FakeJni::JClass const> clazz, std::vector<NativeEntry> entries,
                         void *(*symResolver)(const char *));

public:
    JniSupport();

    void registerMinecraftNatives(void *(*symResolver)(const char *));

    void startGame(ANativeActivity_createFunc *activityOnCreate, GameActivity_createFunc *gameCreate,
                   void *stbiLoadFromMemory, void *stbiImageFree);

    void importFile(std::string path);

    void sendUri(std::string uri);

    void stopGame();

    void waitForGameExit();

    void requestExitGame();

    void setLooperRunning(bool running);

    void onWindowCreated(ANativeWindow *window, AInputQueue *inputQueue);

    void onWindowClosed();

    void onWindowResized(int newWidth, int newHeight);

    void onSetTextboxText(std::string const &text);

    void onCaretPosition(int pos);

    void onReturnKeyPressed();

    void onBackPressed();

    void setGameControllerConnected(int devId, bool connected);

    JavaVM* getJavaVM() { return static_cast<JavaVM*>(&vm); }

    /// Initialize the activity for FakeLooper callbacks (used when Rust JniSupport
    /// handles game startup). Must be called after constructor completes.
    void initActivity();

    TextInputHandler &getTextInputHandler() { return textInput; }

    void setLastChar(FakeJni::JInt sym);

    void sendKeyDown(const GameActivityKeyEvent *event) {
        gameActivityCallbacks.onKeyDown(&gameActivity, event);
    }

    void sendKeyUp(const GameActivityKeyEvent *event) {
        gameActivityCallbacks.onKeyUp(&gameActivity, event);
    }

    void sendMotionEvent(const GameActivityMotionEvent *event) {
        gameActivityCallbacks.onTouchEvent(&gameActivity, event);
    }

    bool isGameActivityVersion() {
        return isGameActivity;
    }

    void setGameActivityCallbacks(const GameActivityCallbacks &cb) { gameActivityCallbacks = cb; }
    GameActivityCallbacks &getGameActivityCallbacks() { return gameActivityCallbacks; }
    void setGameActivityVersion(bool val) { isGameActivity = val; }

    jobject getActivityRef() { return activityRef; }
    MainActivity* getActivityPtr() { return activity.get(); }
    void setActivityStorageDir(const std::string& dir) { if (activity) activity->storageDirectory = dir; }
    void setActivityStbiLoad(void* fn) { if (activity) activity->stbi_load_from_memory = (decltype(activity->stbi_load_from_memory))fn; }
    void setActivityStbiFree(void* fn) { if (activity) activity->stbi_image_free = (decltype(activity->stbi_image_free))fn; }

    GameActivity *getGameActivity() { return &gameActivity; }
    ANativeActivity *getNativeActivity() { return &nativeActivity; }
    ANativeWindow *getWindow() { return window; }
    void *getInputQueue() { return inputQueue; }
};
