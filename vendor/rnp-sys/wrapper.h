// Wrapper header passed to bindgen. Includes the public RNP C API.
//
// The header is found either via the system include path (Homebrew on macOS
// installs it at /opt/homebrew/include, Linux distros at /usr/include) or via
// the RNP_INCLUDE_DIR environment variable when building against a
// source-checkout of librnp.

#include <rnp/rnp.h>
