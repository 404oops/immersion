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

set(IMMERSION_WINDEPLOYQT_PATH "" CACHE FILEPATH "Explicit path to windeployqt executable")
set(IMMERSION_MACDEPLOYQT_PATH "" CACHE FILEPATH "Explicit path to macdeployqt executable")

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
find_package(Qt6 REQUIRED COMPONENTS Widgets Quick Qml QuickControls2 Core Gui Network OpenGL Svg)

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
    Qt6::Svg
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
    RUNTIME DESTINATION .
)

if(Qt6_VERSION_MAJOR EQUAL 6)
    if(WIN32)
        set(_qt_bin_hints "")
        if(Qt6_DIR)
            list(APPEND _qt_bin_hints "${Qt6_DIR}/../../../bin" "${Qt6_DIR}/../../../../bin")
        endif()
        if(EXISTS "C:/Qt/6.11.0/llvm-mingw_64/bin")
            list(APPEND _qt_bin_hints "C:/Qt/6.11.0/llvm-mingw_64/bin")
        endif()
        if(DEFINED ENV{QTDIR} AND NOT "$ENV{QTDIR}" STREQUAL "")
            list(APPEND _qt_bin_hints "$ENV{QTDIR}/bin")
        endif()

        find_program(IMMERSION_WINDEPLOYQT_EXECUTABLE NAMES windeployqt windeployqt.exe HINTS ${_qt_bin_hints} REQUIRED)
        if(IMMERSION_WINDEPLOYQT_EXECUTABLE)
            # Find the actual qt directory to forcefully resolve plugins
            get_filename_component(_qt_bin_dir_raw "${IMMERSION_WINDEPLOYQT_EXECUTABLE}" DIRECTORY)
            get_filename_component(_qt_root_dir_raw "${_qt_bin_dir_raw}" DIRECTORY)

            install(CODE "
                set(_immersion_exe \"\${CMAKE_INSTALL_PREFIX}/Immersion.exe\")
                
                message(STATUS \"Deploying Qt libraries and plugins to \${CMAKE_INSTALL_PREFIX}...\")
                message(STATUS \"Qt root directory: ${_qt_root_dir_raw}\")
                
                # Find LLVM toolchain directory for runtime dependencies
                set(_llvm_bin_dir \"${CMAKE_CXX_COMPILER}\")
                get_filename_component(_llvm_bin_dir \"\${_llvm_bin_dir}\" DIRECTORY)
                
                # Manually copy Qt libraries and plugins instead of using windeployqt
                # This avoids the 'Unable to find the platform plugin' error
                
                # Core Qt libraries
                set(_qt_libs \"Qt6Core\" \"Qt6Gui\" \"Qt6Network\" \"Qt6Qml\" \"Qt6QmlMeta\" \"Qt6QmlWorkerScript\" \"Qt6Widgets\" \"Qt6QmlModels\" \"Qt6Quick\" \"Qt6QuickControls2\" \"Qt6QuickTemplates2\" \"Qt6QuickLayouts\" \"Qt6OpenGL\" \"Qt6Svg\")
                foreach(_lib \${_qt_libs})
                    set(_libfile \"${_qt_root_dir_raw}/bin/\${_lib}.dll\")
                    if(EXISTS \"\${_libfile}\")
                        file(COPY \"\${_libfile}\" DESTINATION \"\${CMAKE_INSTALL_PREFIX}\" NO_SOURCE_PERMISSIONS)
                        message(STATUS \"Deployed \${_lib}.dll\")
                    endif()
                endforeach()

                # Deploy additional Qt Quick/QML runtime families used by style plugins.
                file(GLOB _qt_extra_family_libs
                    \"${_qt_root_dir_raw}/bin/Qt6Quick*.dll\"
                    \"${_qt_root_dir_raw}/bin/Qt6QuickControls2*.dll\"
                    \"${_qt_root_dir_raw}/bin/Qt6QuickTemplates2*.dll\"
                    \"${_qt_root_dir_raw}/bin/Qt6Qml*.dll\"
                )
                foreach(_extra_libfile \${_qt_extra_family_libs})
                    if(EXISTS \"\${_extra_libfile}\")
                        file(COPY \"\${_extra_libfile}\" DESTINATION \"\${CMAKE_INSTALL_PREFIX}\" NO_SOURCE_PERMISSIONS)
                        get_filename_component(_extra_name \"\${_extra_libfile}\" NAME)
                        message(STATUS \"Deployed \${_extra_name}\")
                    endif()
                endforeach()
                
                # Copy plugins directory
                file(COPY \"${_qt_root_dir_raw}/plugins/platforms\" DESTINATION \"\${CMAKE_INSTALL_PREFIX}/plugins\" NO_SOURCE_PERMISSIONS)
                file(COPY \"${_qt_root_dir_raw}/plugins/imageformats\" DESTINATION \"\${CMAKE_INSTALL_PREFIX}/plugins\" NO_SOURCE_PERMISSIONS)
                file(COPY \"${_qt_root_dir_raw}/plugins/iconengines\" DESTINATION \"\${CMAKE_INSTALL_PREFIX}/plugins\" NO_SOURCE_PERMISSIONS)
                file(COPY \"${_qt_root_dir_raw}/plugins/styles\" DESTINATION \"\${CMAKE_INSTALL_PREFIX}/plugins\" NO_SOURCE_PERMISSIONS)
                file(COPY \"${_qt_root_dir_raw}/plugins/generic\" DESTINATION \"\${CMAKE_INSTALL_PREFIX}/plugins\" NO_SOURCE_PERMISSIONS)
                file(COPY \"${_qt_root_dir_raw}/plugins/qmltooling\" DESTINATION \"\${CMAKE_INSTALL_PREFIX}/plugins\" NO_SOURCE_PERMISSIONS)
                file(COPY \"${_qt_root_dir_raw}/plugins/tls\" DESTINATION \"\${CMAKE_INSTALL_PREFIX}/plugins\" NO_SOURCE_PERMISSIONS)
                file(COPY \"${_qt_root_dir_raw}/plugins/networkinformation\" DESTINATION \"\${CMAKE_INSTALL_PREFIX}/plugins\" NO_SOURCE_PERMISSIONS)
                
                # Copy QML modules (with error handling for permission issues)
                if(EXISTS \"${_qt_root_dir_raw}/qml\")
                    file(COPY \"${_qt_root_dir_raw}/qml\" DESTINATION \"\${CMAKE_INSTALL_PREFIX}\" NO_SOURCE_PERMISSIONS)
                    message(STATUS \"QML modules deployed\")
                endif()
                
                # Copy LLVM/MinGW runtime dependencies from toolchain directory
                set(_runtime_libs \"libc++\" \"libunwind\" \"libgcc_s_seh-1\" \"libstdc++-6\" \"libwinpthread-1\")
                foreach(_lib \${_runtime_libs})
                    set(_libfile \"\${_llvm_bin_dir}/\${_lib}.dll\")
                    if(EXISTS \"\${_libfile}\")
                        file(COPY \"\${_libfile}\" DESTINATION \"\${CMAKE_INSTALL_PREFIX}\")
                        message(STATUS \"Deployed \${_lib}.dll from toolchain\")
                    endif()
                endforeach()
                
                message(STATUS \"Qt deployment complete\")
            ")
        else()
            message(FATAL_ERROR "windeployqt not found! Cannot package for Windows without Qt deployment tools.")
        endif()
    endif()

    if(APPLE)
        set(_qt_bin_hints "")
        if(Qt6_DIR)
            list(APPEND _qt_bin_hints "${Qt6_DIR}/../../../bin" "${Qt6_DIR}/../../../../bin")
        endif()
        if(DEFINED ENV{QTDIR} AND NOT "$ENV{QTDIR}" STREQUAL "")
            list(APPEND _qt_bin_hints "$ENV{QTDIR}/bin")
        endif()
        
        find_program(IMMERSION_MACDEPLOYQT_EXECUTABLE NAMES macdeployqt HINTS ${_qt_bin_hints})
        if(IMMERSION_MACDEPLOYQT_EXECUTABLE)
            # Run macdeployqt directly on the installed bundle inside the staging deployment dir
            install(CODE "
                message(STATUS \"Running macdeployqt on \${CMAKE_INSTALL_PREFIX}/Immersion.app\")
                execute_process(
                    COMMAND \"${IMMERSION_MACDEPLOYQT_EXECUTABLE}\" 
                            \"\${CMAKE_INSTALL_PREFIX}/Immersion.app\" 
                            \"-qmldir=${CMAKE_SOURCE_DIR}/src/qml\" 
                            \"-libpath=/opt/homebrew/opt/qtsvg/lib\" 
                            \"-libpath=/usr/local/opt/qtsvg/lib\"
                    RESULT_VARIABLE deploy_res
                )
                if(NOT deploy_res EQUAL 0)
                    message(WARNING \"macdeployqt failed with exit code \${deploy_res}\")
                endif()
            ")
        endif()

        # Codesign the final installed bundle
        install(CODE "
            message(STATUS \"Codesigning \${CMAKE_INSTALL_PREFIX}/Immersion.app...\")
            execute_process(COMMAND /usr/bin/codesign --force --deep --sign - \"\${CMAKE_INSTALL_PREFIX}/Immersion.app\")
        ")
    endif()
endif()

set(CPACK_PACKAGE_NAME "Immersion")
set(CPACK_PACKAGE_VENDOR "Immersion")
set(CPACK_PACKAGE_VERSION ${PROJECT_VERSION})
set(CPACK_PACKAGE_FILE_NAME "Immersion-${PROJECT_VERSION}-${CMAKE_SYSTEM_NAME}")

if(APPLE)
    set(CPACK_GENERATOR "DragNDrop")
elseif(WIN32)
    find_program(MAKENSIS_EXECUTABLE
        NAMES makensis makensis.exe
        HINTS
            "C:/Program Files (x86)/NSIS"
            "C:/Program Files/NSIS"
        PATH_SUFFIXES .
    )
    if(MAKENSIS_EXECUTABLE)
        set(CPACK_NSIS_EXECUTABLE "${MAKENSIS_EXECUTABLE}")
        message(STATUS "Using NSIS at: ${MAKENSIS_EXECUTABLE}")
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
        COMMAND "${CMAKE_CPACK_COMMAND}" -C "${CMAKE_BUILD_TYPE}"
        WORKING_DIRECTORY "${CMAKE_BINARY_DIR}"
        DEPENDS Immersion
        COMMENT "Creating distributable package with CPack"
    )

    add_custom_target(dist-release
        COMMAND "${CMAKE_CPACK_COMMAND}" -C Release
        WORKING_DIRECTORY "${CMAKE_BINARY_DIR}"
        DEPENDS Immersion
        COMMENT "Creating Release package with CPack"
    )
endif()

