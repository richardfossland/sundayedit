# Portable ggml CPU build for CI — injected via CMAKE_PROJECT_TOP_LEVEL_INCLUDES.
#
# GitHub's macos-latest runner is an M1 (no i8mm), but its Xcode mis-detects
# i8mm under `-mcpu=native`: the macro probe defines __ARM_FEATURE_MATMUL_INT8
# while the actual compile flags carry `+noi8mm`, so ggml-cpu-quants.c tries to
# inline the i8mm intrinsic `vmmlaq_s32` into a TU built without i8mm and the
# build dies ("always_inline ... requires target feature 'i8mm'").
#
# Disabling GGML_NATIVE drops `-mcpu=native` so feature detection and the actual
# compile share one baseline target — it builds on the runner AND runs on every
# Apple Silicon chip (incl. M1 / M1 Pro), trading a little inference speed.
#
# This runs right after whisper.cpp's first project() call, before ggml's
# `option(GGML_NATIVE ...)`. A FORCE cache entry survives option() (which never
# overrides an existing cache value) — unlike the WHISPER_NATIVE=OFF alias,
# which option() clobbered back to ON.
set(GGML_NATIVE OFF CACHE BOOL "" FORCE)
