cmake_minimum_required(VERSION 3.21)
project(Immersion VERSION 0.1.0 LANGUAGES CXX)

if(NOT CMAKE_CONFIGURATION_TYPES AND NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE MinSizeRel CACHE STRING "Build type" FORCE)
endif()

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTOUIC ON)
set(CMAKE_AUTORCC ON)

if(WIN32 AND NOT Qt6_DIR)
    set(_qt6_candidates "")

    if(DEFINED ENV{Qt6_DIR} AND NOT "$ENV{Qt6_DIR}" STREQUAL "")
        list(APPEND _qt6_candidates
            "$ENV{Qt6_DIR}"
            "$ENV{Qt6_DIR}/lib/cmake/Qt6"
        )
    endif()

    if(DEFINED ENV{QTDIR} AND NOT "$ENV{QTDIR}" STREQUAL "")
        list(APPEND _qt6_candidates "$ENV{QTDIR}/lib/cmake/Qt6")
    endif()

    file(GLOB _qt_versions LIST_DIRECTORIES true "C:/Qt/6.*")
    list(SORT _qt_versions COMPARE NATURAL ORDER DESCENDING)
    foreach(_qt_version IN LISTS _qt_versions)
        list(APPEND _qt6_candidates
            "${_qt_version}/llvm-mingw_64/lib/cmake/Qt6"
            "${_qt_version}/mingw_64/lib/cmake/Qt6"
            "${_qt_version}/clang_64/lib/cmake/Qt6"
            "${_qt_version}/msvc2022_64/lib/cmake/Qt6"
            "${_qt_version}/msvc2019_64/lib/cmake/Qt6"
            "${_qt_version}/msvc2022_arm64/lib/cmake/Qt6"
        )
    endforeach()

    foreach(_qt6_candidate IN LISTS _qt6_candidates)
        if(EXISTS "${_qt6_candidate}/Qt6Config.cmake")
            set(Qt6_DIR "${_qt6_candidate}" CACHE PATH "Path to Qt6 package config" FORCE)
            message(STATUS "Auto-detected Qt6_DIR: ${Qt6_DIR}")
            break()
        endif()
    endforeach()
endif()

if(Qt6_DIR)
    list(PREPEND CMAKE_PREFIX_PATH "${Qt6_DIR}")
endif()

if(WIN32)
    set(_using_msvc_abi FALSE)
    if(MSVC OR CMAKE_CXX_SIMULATE_ID STREQUAL "MSVC")
        set(_using_msvc_abi TRUE)
    endif()
    if(CMAKE_CXX_COMPILER_TARGET MATCHES "msvc")
        set(_using_msvc_abi TRUE)
    endif()
    if(_using_msvc_abi)
        message(FATAL_ERROR
            "MSVC ABI is not supported in this project on Windows.\n"
            "Use clang/clang++ with GNU/MinGW ABI (Ninja recommended).\n"
            "Examples:\n"
            "  CMAKE_C_COMPILER=C:/Qt/6.11.0/llvm-mingw_64/bin/clang.exe\n"
            "  CMAKE_CXX_COMPILER=C:/Qt/6.11.0/llvm-mingw_64/bin/clang++.exe\n"
            "Current compiler: ${CMAKE_CXX_COMPILER}\n"
            "Current compiler target: ${CMAKE_CXX_COMPILER_TARGET}")
    endif()
endif()

if(WIN32 AND Qt6_DIR)
    set(_qt6_is_mingw FALSE)
    set(_qt6_is_msvc FALSE)
    if(Qt6_DIR MATCHES "[\\/](llvm-mingw_64|mingw_64)[\\/]")
        set(_qt6_is_mingw TRUE)
    endif()
    if(Qt6_DIR MATCHES "[\\/]msvc[0-9_]+[\\/]")
        set(_qt6_is_msvc TRUE)
    endif()

    set(_using_mingw_abi FALSE)
    if(CMAKE_CXX_COMPILER_TARGET MATCHES "mingw|gnu")
        set(_using_mingw_abi TRUE)
    endif()

    if(_qt6_is_mingw AND _using_msvc_abi)
        message(FATAL_ERROR
            "Incompatible toolchain/Qt combination detected.\n"
            "Qt6_DIR points to a MinGW Qt kit: ${Qt6_DIR}\n"
            "but the compiler targets MSVC ABI.\n"
            "Use one of these combinations:\n"
            "  1) MinGW Qt + MinGW/LLVM-MinGW compiler (for example C:/Qt/6.11.0/llvm-mingw_64/bin/clang++.exe)\n"
            "  2) MSVC Qt kit + MSVC/clang-cl compiler\n"
            "Current compiler target: ${CMAKE_CXX_COMPILER_TARGET}")
    endif()

    if(_qt6_is_msvc AND _using_mingw_abi)
        message(FATAL_ERROR
            "Incompatible toolchain/Qt combination detected.\n"
            "Qt6_DIR points to an MSVC Qt kit: ${Qt6_DIR}\n"
            "but the compiler targets MinGW/GNU ABI.\n"
            "Use a MinGW/LLVM-MinGW Qt kit with clang/clang++.\n"
            "Current compiler target: ${CMAKE_CXX_COMPILER_TARGET}")
    endif()
endif()

find_package(Qt6 REQUIRED COMPONENTS Widgets Quick Qml QuickControls2)

add_executable(Immersion
    src/main.cpp
    src/ui/MainWindow.cpp
    src/ui/MainWindow.h
    src/ui/QmlBackend.cpp
    src/ui/QmlBackend.h
    src/core/FileEvent.h
    src/core/IFileWatcher.h
    src/core/FileWatcherFactory.cpp
    src/core/FileWatcherFactory.h
    src/core/ProjectConfig.cpp
    src/core/ProjectConfig.h
    src/core/ProjectDiscovery.cpp
    src/core/ProjectDiscovery.h
    src/core/SnapshotService.cpp
    src/core/SnapshotService.h
    src/persistence/ObjectStore.cpp
    src/persistence/ObjectStore.h
    src/persistence/MetadataStore.cpp
    src/persistence/MetadataStore.h
    src/persistence/ProjectRegistry.cpp
    src/persistence/ProjectRegistry.h
)

qt_add_resources(Immersion Immersion_qml
    PREFIX "/"
    FILES
    src/qml/App.qml
        src/qml/Main.qml
        src/qml/Theme.qml
        src/qml/VersionManagerWindow.qml
    src/qml/MainView.ui.qml
)

set_target_properties(Immersion PROPERTIES
    MACOSX_BUNDLE ON
    MACOSX_BUNDLE_GUI_IDENTIFIER "com.Immersion.app"
    MACOSX_BUNDLE_BUNDLE_NAME "Immersion"
    MACOSX_BUNDLE_BUNDLE_VERSION "${PROJECT_VERSION}"
    MACOSX_BUNDLE_SHORT_VERSION_STRING "${PROJECT_VERSION}"
)

target_link_libraries(Immersion
    Qt6::Widgets
    Qt6::Quick
    Qt6::Qml
    Qt6::QuickControls2
)

set_target_properties(Immersion PROPERTIES INTERPROCEDURAL_OPTIMIZATION_MINSIZEREL ON)

target_compile_options(Immersion PRIVATE
    $<$<AND:$<CONFIG:MinSizeRel>,$<NOT:$<CXX_COMPILER_ID:MSVC>>>:-Os>
    $<$<AND:$<CONFIG:MinSizeRel>,$<NOT:$<CXX_COMPILER_ID:MSVC>>>:-ffunction-sections>
    $<$<AND:$<CONFIG:MinSizeRel>,$<NOT:$<CXX_COMPILER_ID:MSVC>>>:-fdata-sections>
)

target_link_options(Immersion PRIVATE
    $<$<CONFIG:MinSizeRel>:-Wl,-dead_strip>
)

target_compile_options(Immersion PRIVATE
    $<$<CXX_COMPILER_ID:MSVC>:/W4 /permissive->
    $<$<NOT:$<CXX_COMPILER_ID:MSVC>>:-Wall -Wextra -Wpedantic>
)

include(GNUInstallDirs)
install(TARGETS Immersion
    BUNDLE DESTINATION .
    RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR}
)

set(CPACK_PACKAGE_NAME "Immersion")
set(CPACK_PACKAGE_VENDOR "Immersion")
set(CPACK_PACKAGE_VERSION ${PROJECT_VERSION})
set(CPACK_PACKAGE_FILE_NAME "Immersion-${PROJECT_VERSION}-${CMAKE_SYSTEM_NAME}")

if(APPLE)
    set(CPACK_GENERATOR "DragNDrop")
elseif(WIN32)
    set(CPACK_GENERATOR "NSIS")
endif()

include(CPack)
