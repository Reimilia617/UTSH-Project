# CMakeLists.txt — utsh-ffi C++ 侧**可选**的独立构建。
#
# 说明：
#   * 常规开发走 cargo（crates/utsh-ffi/build.rs 用 cc crate 编译 C++），
#     不需要 CMake；
#   * 本文件用于只改 C++ 时的快速验证，或为 IDE（CLion/VS Code CMake Tools）
#     提供编译数据库；
#   * Phase 1 接入 Bison/Flex 后，在这里添加 grammar.tab.cc / lexer.yy.cc 的
#     生成规则（下方已留 TODO）。

cmake_minimum_required(VERSION 3.10)
project(utsh_ffi_cpp LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

option(UTSH_ENABLE_READLINE "Build the GNU Readline wrapper" OFF)

set(UTSH_CPP_SRC
    ${CMAKE_CURRENT_SOURCE_DIR}/src/cpp/bash_parser.cpp
)

if(UTSH_ENABLE_READLINE)
    find_package(Readline QUIET)
    if(Readline_FOUND)
        list(APPEND UTSH_CPP_SRC ${CMAKE_CURRENT_SOURCE_DIR}/src/cpp/readline_wrapper.cpp)
    else()
        message(FATAL_ERROR "UTSH_ENABLE_READLINE=ON but GNU Readline was not found "
                            "(Debian/Ubuntu: apt install libreadline-dev)")
    endif()
endif()

add_library(utsh_ffi_cpp STATIC ${UTSH_CPP_SRC})
target_include_directories(utsh_ffi_cpp PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/src/cpp)

if(UTSH_ENABLE_READLINE AND Readline_FOUND)
    target_link_libraries(utsh_ffi_cpp PUBLIC Readline::Readline)
endif()

# ---------------------------------------------------------------------------
# TODO(Phase 1): Bison / Flex 生成（需要 bison 3.0+、flex 2.6+）
#
# find_program(BISON_EXECUTABLE bison REQUIRED)
# find_program(FLEX_EXECUTABLE flex REQUIRED)
#
# add_custom_command(
#     OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/grammar.tab.cc
#            ${CMAKE_CURRENT_BINARY_DIR}/grammar.tab.hh
#     COMMAND ${BISON_EXECUTABLE} -d -o ${CMAKE_CURRENT_BINARY_DIR}/grammar.tab.cc
#             ${CMAKE_CURRENT_SOURCE_DIR}/src/cpp/grammar.y
#     DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/src/cpp/grammar.y)
#
# add_custom_command(
#     OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/lexer.yy.cc
#     COMMAND ${FLEX_EXECUTABLE} -o ${CMAKE_CURRENT_BINARY_DIR}/lexer.yy.cc
#             ${CMAKE_CURRENT_SOURCE_DIR}/src/cpp/lexer.l
#     DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/src/cpp/lexer.l)
#
# 然后把生成的 .cc 追加到 UTSH_CPP_SRC 并替换/扩展 bash_parser.cpp 的
# parse_bash() 内部实现（见 grammar.y 头注释）。
# ---------------------------------------------------------------------------
