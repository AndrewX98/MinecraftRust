#include <file_picker.h>
#include <file_picker_factory.h>
#include <cstring>
#include <memory>
#include <string>
#include <vector>

extern "C" {
    void* rust_filepicker_create();
    void rust_filepicker_set_title(void* picker, const char* title);
    void rust_filepicker_set_filename(void* picker, const char* name);
    void rust_filepicker_set_mode(void* picker, int mode);
    void rust_filepicker_set_filters(void* picker, const char* const* patterns, int count);
    bool rust_filepicker_show(void* picker);
    const char* rust_filepicker_get_picked_file(void* picker);
    void rust_filepicker_destroy(void* picker);
}

class RustFilePickerWrapper : public FilePicker {
    void* inner;
    std::string cachedPickedFile;
public:
    RustFilePickerWrapper() : inner(rust_filepicker_create()) {}
    ~RustFilePickerWrapper() override { rust_filepicker_destroy(inner); }

    void setTitle(const std::string& title) override {
        rust_filepicker_set_title(inner, title.c_str());
    }

    void setFileName(const std::string& name) override {
        rust_filepicker_set_filename(inner, name.c_str());
    }

    void setMode(Mode mode) override {
        rust_filepicker_set_mode(inner, static_cast<int>(mode));
    }

    void setFileNameFilters(const std::vector<std::string>& patterns) override {
        std::vector<const char*> cstrs;
        cstrs.reserve(patterns.size());
        for (auto& p : patterns) {
            cstrs.push_back(p.c_str());
        }
        rust_filepicker_set_filters(inner, cstrs.data(), static_cast<int>(cstrs.size()));
    }

    bool show() override {
        bool ok = rust_filepicker_show(inner);
        if (ok) {
            const char* s = rust_filepicker_get_picked_file(inner);
            cachedPickedFile = s ? s : "";
        }
        return ok;
    }

    std::string getPickedFile() const override {
        return cachedPickedFile;
    }
};

std::unique_ptr<FilePicker> FilePickerFactory::createFilePicker() {
    return std::make_unique<RustFilePickerWrapper>();
}
