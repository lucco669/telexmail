/*
 * Declaration-only getopt/getopt_long shim for MSVC.
 *
 * rnp-src builds only the `librnp` target; librnp's own translation units
 * include <getopt.h> (via src/common/uniwin.h -> getoptwin.h) but never
 * call getopt functions — only the rnp/rnpkeys CLIs do, and those targets
 * are never built here. Upstream rnp satisfies this include on MSVC from
 * vcpkg's getopt-win32 port, which is LGPL and therefore not vendored
 * here. This header declares the standard POSIX/GNU interface so the
 * library sources compile; it provides no implementation. It is covered
 * by the same BSD-2-Clause license as the rest of this crate.
 */
#ifndef RNP_SRC_COMPAT_GETOPT_H
#define RNP_SRC_COMPAT_GETOPT_H

#ifdef __cplusplus
extern "C" {
#endif

extern char *optarg;
extern int optind;
extern int opterr;
extern int optopt;

int getopt(int argc, char *const argv[], const char *optstring);

#ifndef RNP_SRC_COMPAT_GETOPT_LONG_ONLY_DECLS
#define RNP_SRC_COMPAT_GETOPT_LONG_ONLY_DECLS

enum
{
    no_argument = 0,
    required_argument = 1,
    optional_argument = 2
};

struct option
{
    const char *name;
    int has_arg;
    int *flag;
    int val;
};

int getopt_long(int argc,
                char *const argv[],
                const char *optstring,
                const struct option *longopts,
                int *longindex);
int getopt_long_only(int argc,
                     char *const argv[],
                     const char *optstring,
                     const struct option *longopts,
                     int *longindex);

#endif /* RNP_SRC_COMPAT_GETOPT_LONG_ONLY_DECLS */

#ifdef __cplusplus
}
#endif

#endif /* RNP_SRC_COMPAT_GETOPT_H */
