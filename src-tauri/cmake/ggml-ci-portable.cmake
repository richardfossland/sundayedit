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

# x86_64 slice of the universal macOS build: pin the instruction baseline
# explicitly. The Rust `cmake` crate marks the arm64→x86_64 cross-compile with
# CMAKE_SYSTEM_NAME, so ggml sees CMAKE_CROSSCOMPILING and would default every
# x86 SIMD option OFF (a scalar build — far too slow); a non-cross configure
# would instead default them ALL on, including AVX2/FMA (Haswell-only —
# SIGILLs on the 2012 Ivy Bridge Macs and the 2013 Mac Pro that our 10.15
# minimum still supports). Pin the set every macOS-10.15-capable Intel Mac
# has (Ivy Bridge, 2012+): SSE4.2 + AVX + F16C, no AVX2/FMA/BMI2.
# ggml only applies these on x86 targets, so they're inert for the arm64 slice.
set(GGML_SSE42 ON CACHE BOOL "" FORCE)
set(GGML_AVX ON CACHE BOOL "" FORCE)
set(GGML_F16C ON CACHE BOOL "" FORCE)
set(GGML_AVX2 OFF CACHE BOOL "" FORCE)
set(GGML_FMA OFF CACHE BOOL "" FORCE)
set(GGML_BMI2 OFF CACHE BOOL "" FORCE)
