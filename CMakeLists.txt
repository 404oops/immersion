cmake_minimum_required(VERSION 3.21)
project(Immersion VERSION 0.1.0 LANGUAGES CXX)

if(NOT CMAKE_CONFIGURATION_TYPES AND NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE MinSizeRel CACHE STRING "Build type" FORCE)
endif()

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

if(WIN32 AND NOT MSVC)
    set(CMAKE_EXE_LINKER_FLAGS "${CMAKE_EXE_LINKER_FLAGS} -static -static-libgcc -static-libstdc++" CACHE STRING "EXE linker flags" FORCE)
elseif(UNIX)
    set(CMAKE_EXE_LINKER_FLAGS "${CMAKE_EXE_LINKER_FLAGS} -static-libgcc -static-libstdc++" CACHE STRING "EXE linker flags" FORCE)
endif()

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

set(BUILD_SHARED_LIBS OFF CACHE BOOL "Build as static library")
set(Qt6_STATIC TRUE)

# Additional Qt6 configuration for static linking
set(Qt6_NO_UNINSTALLED_BUILD_SUPPORT ON)

if(WIN32)
    # For MinGW/LLVM-MinGW on Windows: ensure static linkage, not shared DLLs
    # These variables affect how Qt6Config.cmake selects libraries
    set(Qt6_SHARED_LIBS OFF)
    set(Qt6_USE_STATIC_LIBS ON)
    
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

    if(_qt6_is_msvc)
        message(FATAL_ERROR
            "MSVC Qt kits are not supported (MSVC ABI rejected at top-level check).
            Use MinGW/LLVM-MinGW Qt kit with clang/clang++.
            Qt6_DIR: ${Qt6_DIR}")
    endif()
endif()

# Force static library prioritization for MinGW/LLVM-MinGW on Windows
if(WIN32 AND NOT MSVC)
    # Order: static .a first, then import libs .lib, then shared .dll
    set(CMAKE_FIND_LIBRARY_SUFFIXES ".a" ".lib" ".dll")
    message(STATUS "Configured CMAKE_FIND_LIBRARY_SUFFIXES for static linking: ${CMAKE_FIND_LIBRARY_SUFFIXES}")
endif()

find_package(Qt6 REQUIRED COMPONENTS Widgets Quick Qml QuickControls2 Core Gui Network OpenGL)

# Verify Qt6 is static (check for .a files in Qt6_DIR)
if(WIN32 AND Qt6_DIR)
    if(EXISTS "${Qt6_DIR}/../libQt6Core.a")
        message(STATUS "✓ Qt6Core static library found (.a)")
    elseif(EXISTS "${Qt6_DIR}/../../../lib/libQt6Core.a")
        message(STATUS "✓ Qt6Core static library found (.a)")
    else()
        message(WARNING "Qt6Core static library (.a) not found - shared DLL might be used!")
        if(EXISTS "${Qt6_DIR}/../Qt6Core.dll")
            message(FATAL_ERROR 
                "ERROR: Found Qt6Core.dll (shared) instead of libQt6Core.a (static)!\n"
                "The Qt kit must have static libraries. Check your Qt installation at:\n"
                "  ${Qt6_DIR}\n"
                "You need llvm-mingw_64 or mingw_64 kit with static library (.a) files,\n"
                "not a shared library (.dll) build.")
        endif()
    endif()
endif()

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
    Qt6::Core
    Qt6::Gui
    Qt6::Network
    Qt6::OpenGL
)

# Ensure pure static linking on Windows with MinGW/Clang
if(WIN32 AND NOT MSVC)
    target_link_options(Immersion PRIVATE -static -static-libgcc -static-libstdc++)
    message(STATUS "✓ Applied full static linking flags to Immersion target")
endif()

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
