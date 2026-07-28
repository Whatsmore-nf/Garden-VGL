cmake_minimum_required(VERSION 3.15)
project(vgl VERSION 1.0.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# 选项
option(BUILD_EXAMPLES "Build example programs" OFF)

# 源文件
set(VGL_SOURCES
    src/main.cpp
    src/lexer.cpp
    src/parser.cpp
    src/canvas.cpp
)

# 头文件
set(VGL_HEADERS
    include/vgl.hpp
    include/lexer.hpp
    include/parser.hpp
    include/canvas.hpp
)

# 创建可执行文件
add_executable(vgl ${VGL_SOURCES} ${VGL_HEADERS})

target_include_directories(vgl PRIVATE ${CMAKE_SOURCE_DIR}/include)

# 链接 PNG 库用于输出
find_package(PNG REQUIRED)
target_link_libraries(vgl PRIVATE PNG::PNG)

# 安装规则
install(TARGETS vgl DESTINATION bin)
