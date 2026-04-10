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

set(BUILD_SHARED_LIBS ON CACHE BOOL "Build with shared libraries by default")

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

if(WIN32 AND Qt6_DIR AND Qt6_DIR MATCHES "[\\/]msvc[0-9_]+[\\/]")
    message(FATAL_ERROR
        "MSVC Qt kits are not supported (MSVC ABI rejected at top-level check).\n"
        "Use MinGW/LLVM-MinGW Qt kit with clang/clang++.\n"
        "Qt6_DIR: ${Qt6_DIR}")
endif()

find_package(Threads REQUIRED)
find_package(Qt6 REQUIRED COMPONENTS Widgets Quick Qml QuickControls2 Core Gui Network OpenGL)

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
    WIN32_EXECUTABLE $<BOOL:WIN32>
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

if(QT_VERSION_MAJOR EQUAL 6)
    if(WIN32)
        set(_qt_bin_hints "")
        if(Qt6_DIR)
            list(APPEND _qt_bin_hints
                "${Qt6_DIR}/../../../bin"
                "${Qt6_DIR}/../../../../bin"
            )
        endif()
        if(DEFINED ENV{QTDIR} AND NOT "$ENV{QTDIR}" STREQUAL "")
            list(APPEND _qt_bin_hints "$ENV{QTDIR}/bin")
        endif()

        find_program(IMMERSION_WINDEPLOYQT_EXECUTABLE
            NAMES windeployqt windeployqt.exe
            HINTS ${_qt_bin_hints}
        )

        if(IMMERSION_WINDEPLOYQT_EXECUTABLE)
                        set(_immersion_windeployqt_install_code [=[
set(_immersion_install_bin "$ENV{DESTDIR}${CMAKE_INSTALL_PREFIX}/@CMAKE_INSTALL_BINDIR@")
set(_immersion_exe "${_immersion_install_bin}/Immersion.exe")
if(EXISTS "${_immersion_exe}")
    message(STATUS "Running windeployqt for: ${_immersion_exe}")
    if("${CMAKE_INSTALL_CONFIG_NAME}" STREQUAL "Debug")
        set(_immersion_cfg_flag --debug)
    else()
        set(_immersion_cfg_flag --release)
    endif()

    execute_process(
        COMMAND "@IMMERSION_WINDEPLOYQT_EXECUTABLE@" ${_immersion_cfg_flag} --dir "${_immersion_install_bin}" --qmldir "@CMAKE_SOURCE_DIR@/src/qml" --no-translations "${_immersion_exe}"
        RESULT_VARIABLE _immersion_windeployqt_result
    )

    if(NOT _immersion_windeployqt_result EQUAL 0)
        message(FATAL_ERROR "windeployqt failed during install with exit code: ${_immersion_windeployqt_result}")
    endif()
    message(STATUS "windeployqt completed successfully")
endif()
]=])
                        string(CONFIGURE "${_immersion_windeployqt_install_code}" _immersion_windeployqt_install_code @ONLY)
                        install(CODE "${_immersion_windeployqt_install_code}")
        else()
            message(WARNING "windeployqt was not found at configure time; Windows package may miss Qt runtime dependencies.")
        endif()
    elseif(COMMAND qt_generate_deploy_qml_app_script)
        qt_generate_deploy_qml_app_script(
            TARGET Immersion
            OUTPUT_SCRIPT immersion_deploy_script
            QML_DIR "${CMAKE_SOURCE_DIR}/src/qml"
            NO_UNSUPPORTED_PLATFORM_ERROR
        )
        install(SCRIPT ${immersion_deploy_script})
    elseif(COMMAND qt_generate_deploy_app_script)
        qt_generate_deploy_app_script(
            TARGET Immersion
            OUTPUT_SCRIPT immersion_deploy_script
            NO_UNSUPPORTED_PLATFORM_ERROR
        )
        install(SCRIPT ${immersion_deploy_script})
    endif()
endif()

set(CPACK_PACKAGE_NAME "Immersion")
set(CPACK_PACKAGE_VENDOR "Immersion")
set(CPACK_PACKAGE_VERSION ${PROJECT_VERSION})
set(CPACK_PACKAGE_FILE_NAME "Immersion-${PROJECT_VERSION}-${CMAKE_SYSTEM_NAME}")

if(APPLE)
    set(CPACK_GENERATOR "DragNDrop")
elseif(WIN32)
    find_program(MAKENSIS_EXECUTABLE makensis)
    if(MAKENSIS_EXECUTABLE)
        set(CPACK_GENERATOR "NSIS;ZIP")
    else()
        message(STATUS "NSIS not found, packaging with ZIP only")
        set(CPACK_GENERATOR "ZIP")
    endif()
endif()

include(CPack)

# CMake-native packaging entrypoints (no external scripts required)
if(CMAKE_CONFIGURATION_TYPES)
    add_custom_target(dist
        COMMAND "${CMAKE_CPACK_COMMAND}" -C Release
        WORKING_DIRECTORY "${CMAKE_BINARY_DIR}"
        DEPENDS Immersion
        COMMENT "Creating Release distributable package with CPack"
    )
    add_custom_target(dist-debug
        COMMAND "${CMAKE_CPACK_COMMAND}" -C Debug
        WORKING_DIRECTORY "${CMAKE_BINARY_DIR}"
        DEPENDS Immersion
        COMMENT "Creating Debug package with CPack"
    )
    add_custom_target(dist-relwithdebinfo
        COMMAND "${CMAKE_CPACK_COMMAND}" -C RelWithDebInfo
        WORKING_DIRECTORY "${CMAKE_BINARY_DIR}"
        DEPENDS Immersion
        COMMENT "Creating RelWithDebInfo package with CPack"
    )
else()
    add_custom_target(dist
        COMMAND "${CMAKE_CPACK_COMMAND}"
        WORKING_DIRECTORY "${CMAKE_BINARY_DIR}"
        DEPENDS Immersion
        COMMENT "Creating distributable package with CPack"
    )
endif()

if(CMAKE_CONFIGURATION_TYPES)
    add_custom_target(dist-release
        COMMAND "${CMAKE_CPACK_COMMAND}" -C Release
        WORKING_DIRECTORY "${CMAKE_BINARY_DIR}"
        DEPENDS Immersion
        COMMENT "Creating Release package with CPack"
    )
endif()
