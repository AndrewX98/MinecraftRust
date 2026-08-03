#pragma once

#include <daemon_utils/daemon_launcher.h>

// EnvPathUtil is ported to Rust (crates/util/src/file_util.rs).
extern "C" const char* env_path_util_get_data_home();

namespace msa {
namespace client {

class ServiceClient;

class ServiceLauncher : public daemon_utils::daemon_launcher {

private:
    std::string data_path;
    std::string executable_path;

    static std::string getDefaultDataPath() {
        return std::string(env_path_util_get_data_home()) + "/msa";
    }

public:
    ServiceLauncher(std::string const& executable_path, std::string const& data_path = getDefaultDataPath()) :
            daemon_launcher(data_path + "/service"), data_path(data_path), executable_path(executable_path) {}

    std::vector<std::string> get_arguments() override {
        return {executable_path, "-d", data_path, "-x"};
    }

};

}
}