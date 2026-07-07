cmake_minimum_required(VERSION 3.21)
project(Immersion VERSION 0.1.0 LANGUAGES CXX)

if(NOT CMAKE_CONFIGURATION_TYPES AND NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE MinSizeRel CACHE STRING "Build type" FORCE)
endif()

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

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
# Only link what the C++ code uses. QtQuick/QtNetwork are pulled in at
# runtime by the QML engine; linking extra modules here forces them (and
# their plugins) into every deployed package.
find_package(Qt6 REQUIRED COMPONENTS Qml Gui OpenGL QuickControls2 Widgets Network)

add_executable(Immersion
    src/main.cpp
    src/ui/QmlBackend.cpp
    src/ui/QmlBackend.h
    src/ui/TrayController.cpp
    src/ui/TrayController.h
    src/ui/SingleInstanceGuard.cpp
    src/ui/SingleInstanceGuard.h
    src/ui/PlatformAgent.h
    src/core/BackupTemplate.cpp
    src/core/BackupTemplate.h
    src/core/FileEvent.h
    src/core/IFileWatcher.h
    src/core/FileWatcherFactory.cpp
    src/core/FileWatcherFactory.h
    src/core/ProjectConfig.cpp
    src/core/ProjectConfig.h
    src/core/ProjectDiscovery.cpp
    src/core/ProjectDiscovery.h
    src/core/ProjectDiscoveryScanWorker.cpp
    src/core/ProjectDiscoveryScanWorker.h
    src/core/SnapshotService.cpp
    src/core/SnapshotService.h
    src/core/VersionId.h
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
        src/qml/SettingsWindow.qml
        src/qml/MainView.ui.qml
        src/qml/OnboardingView.qml
        src/qml/ActionButton.qml
        src/qml/ThemedTextField.qml
        src/qml/ThemedComboBox.qml
        src/qml/ThemedSlider.qml
        src/qml/ThemedSwitch.qml
        src/qml/ThemedSpinBox.qml
        src/qml/PanelButton.qml
        src/qml/ThemedTextArea.qml
        src/qml/VersionGraphNode.qml
)

set_target_properties(Immersion PROPERTIES
    # Only affects Windows; avoids a console window popping up with the GUI.
    WIN32_EXECUTABLE ON
    MACOSX_BUNDLE ON
    MACOSX_BUNDLE_GUI_IDENTIFIER "com.Immersion.app"
    MACOSX_BUNDLE_BUNDLE_NAME "Immersion"
    MACOSX_BUNDLE_BUNDLE_VERSION "${PROJECT_VERSION}"
    MACOSX_BUNDLE_SHORT_VERSION_STRING "${PROJECT_VERSION}"
)

target_link_libraries(Immersion
    Qt6::Qml
    Qt6::Gui
    Qt6::OpenGL
    Qt6::QuickControls2
    Qt6::Widgets
    Qt6::Network
)

if(APPLE)
    target_sources(Immersion PRIVATE
        src/ui/MacStatusBar.h
        src/ui/PlatformAgent_mac.mm
        src/ui/MacStatusBar.mm
    )
    target_link_libraries(Immersion
        "-framework AppKit"
        "-framework UserNotifications"
        "-framework ServiceManagement"
    )
elseif(WIN32)
    target_sources(Immersion PRIVATE src/ui/PlatformAgent_win.cpp)
else()
    target_sources(Immersion PRIVATE src/ui/PlatformAgent.cpp)
endif()

set_target_properties(Immersion PROPERTIES INTERPROCEDURAL_OPTIMIZATION_MINSIZEREL ON)

target_compile_options(Immersion PRIVATE
    $<$<AND:$<CONFIG:MinSizeRel>,$<NOT:$<CXX_COMPILER_ID:MSVC>>>:-Os>
    $<$<AND:$<CONFIG:MinSizeRel>,$<NOT:$<CXX_COMPILER_ID:MSVC>>>:-ffunction-sections>
    $<$<AND:$<CONFIG:MinSizeRel>,$<NOT:$<CXX_COMPILER_ID:MSVC>>>:-fdata-sections>
)

# -dead_strip is an Apple linker flag; GNU/LLD linkers reject it and would
# break MinSizeRel builds on Windows/Linux (MinSizeRel is the default type).
if(APPLE)
    target_link_options(Immersion PRIVATE
        $<$<CONFIG:MinSizeRel>:-Wl,-dead_strip>
    )
elseif(NOT MSVC)
    target_link_options(Immersion PRIVATE
        $<$<CONFIG:MinSizeRel>:-Wl,--gc-sections>
    )
endif()

target_compile_options(Immersion PRIVATE
    $<$<CXX_COMPILER_ID:MSVC>:/W4 /permissive->
    $<$<NOT:$<CXX_COMPILER_ID:MSVC>>:-Wall -Wextra -Wpedantic>
)

# End-to-end test binary (not part of packaged builds).
option(MUSIT_BUILD_TESTS "Build the end-to-end test binary" OFF)
if(MUSIT_BUILD_TESTS)
    add_executable(musit_e2e_test
        tests/e2e_backup_test.cpp
        src/ui/QmlBackend.cpp
        src/ui/QmlBackend.h
        src/core/BackupTemplate.cpp
        src/core/BackupTemplate.h
        src/core/FileEvent.h
        src/core/IFileWatcher.h
        src/core/FileWatcherFactory.cpp
        src/core/FileWatcherFactory.h
        src/core/ProjectConfig.cpp
        src/core/ProjectConfig.h
        src/core/ProjectDiscovery.cpp
        src/core/ProjectDiscovery.h
        src/core/ProjectDiscoveryScanWorker.cpp
        src/core/ProjectDiscoveryScanWorker.h
        src/core/SnapshotService.cpp
        src/core/SnapshotService.h
        src/core/VersionId.h
        src/persistence/ObjectStore.cpp
        src/persistence/ObjectStore.h
        src/persistence/MetadataStore.cpp
        src/persistence/MetadataStore.h
        src/persistence/ProjectRegistry.cpp
        src/persistence/ProjectRegistry.h
    )
    target_link_libraries(musit_e2e_test Qt6::Qml Qt6::Gui)
    target_sources(musit_e2e_test PRIVATE src/ui/PlatformAgent.cpp)
endif()

include(GNUInstallDirs)
set(CPACK_PROJECT_CONFIG_FILE "${CMAKE_SOURCE_DIR}/cmake/ImmersionCPack.cmake")
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

        # Use windeployqt with --qmldir so only the Qt libraries, plugins and
        # QML modules this app actually imports get deployed. The previous
        # manual approach copied Qt's ENTIRE qml/ tree (QtQuick3D, all
        # Controls styles, WebView, ... hundreds of MB of unused assets) and
        # at the same time missed the platforms/ plugin the app needs to boot.
        install(CODE "
            message(STATUS \"Running windeployqt on \${CMAKE_INSTALL_PREFIX}/Immersion.exe\")
            execute_process(
                COMMAND \"${IMMERSION_WINDEPLOYQT_EXECUTABLE}\"
                        --qmldir \"${CMAKE_SOURCE_DIR}/src/qml\"
                        --no-translations
                        --no-system-d3d-compiler
                        --no-opengl-sw
                        --compiler-runtime
                        \"\${CMAKE_INSTALL_PREFIX}/Immersion.exe\"
                RESULT_VARIABLE deploy_res
            )
            if(NOT deploy_res EQUAL 0)
                message(FATAL_ERROR \"windeployqt failed with exit code \${deploy_res}\")
            endif()

            # llvm-mingw runtime DLLs that windeployqt's --compiler-runtime
            # does not know about.
            set(_llvm_bin_dir \"${CMAKE_CXX_COMPILER}\")
            get_filename_component(_llvm_bin_dir \"\${_llvm_bin_dir}\" DIRECTORY)
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
                    RESULT_VARIABLE deploy_res
                )
                if(NOT deploy_res EQUAL 0)
                    message(FATAL_ERROR \"macdeployqt failed with exit code \${deploy_res}\")
                endif()

                # Clean up unused Qt frameworks and plugins to shrink the binary size
                set(frameworks_dir \"\${CMAKE_INSTALL_PREFIX}/Immersion.app/Contents/Frameworks\")
                set(plugins_dir \"\${CMAKE_INSTALL_PREFIX}/Immersion.app/Contents/PlugIns\")

                # 1. Remove unused frameworks (keep QtOpenGL — libqtquick2plugin
                # depends on it even though the app does not call OpenGL directly).
                set(unused_frameworks
                    \"QtSql.framework\"
                    \"QtQuickControls2FluentWinUI3StyleImpl.framework\"
                    \"QtQuickControls2Imagine.framework\"
                    \"QtQuickControls2ImagineStyleImpl.framework\"
                    \"QtQuickControls2Universal.framework\"
                    \"QtQuickControls2UniversalStyleImpl.framework\"
                    \"QtQuickControls2IOSStyleImpl.framework\"
                    \"QtQuickControls2Fusion.framework\"
                    \"QtQuickControls2FusionStyleImpl.framework\"
                    \"QtQuickControls2Material.framework\"
                    \"QtQuickControls2MaterialStyleImpl.framework\"
                    \"QtQuickEffects.framework\"
                    \"QtQuickParticles.framework\"
                    \"QtQuickShapes.framework\"
                    \"QtQuickShapesDesignHelpers.framework\"
                    \"QtQuickVectorImage.framework\"
                    \"QtQuickVectorImageGenerator.framework\"
                    \"QtQuickVectorImageHelpers.framework\"
                )
                foreach(fw IN LISTS unused_frameworks)
                    if(EXISTS \"\${frameworks_dir}/\${fw}\")
                        file(REMOVE_RECURSE \"\${frameworks_dir}/\${fw}\")
                        message(STATUS \"Pruned unused framework: \${fw}\")
                    endif()
                endforeach()

                # 2. Remove unused quick plugins
                set(unused_quick_plugins
                    \"libqtquickcontrols2fluentwinui3styleplugin.dylib\"
                    \"libqtquickcontrols2fluentwinui3styleimplplugin.dylib\"
                    \"libqtquickcontrols2universalstyleplugin.dylib\"
                    \"libqtquickcontrols2universalstyleimplplugin.dylib\"
                    \"libqtquickcontrols2iosstyleplugin.dylib\"
                    \"libqtquickcontrols2iosstyleimplplugin.dylib\"
                    \"libqtquickcontrols2imaginestyleplugin.dylib\"
                    \"libqtquickcontrols2imaginestyleimplplugin.dylib\"
                    \"libqtquickcontrols2fusionstyleplugin.dylib\"
                    \"libqtquickcontrols2fusionstyleimplplugin.dylib\"
                    \"libqtquickcontrols2materialstyleplugin.dylib\"
                    \"libqtquickcontrols2materialstyleimplplugin.dylib\"
                    \"libeffectsplugin.dylib\"
                    \"libparticlesplugin.dylib\"
                    \"libqmlshapesplugin.dylib\"
                    \"libqtquickshapesdesignhelpersplugin.dylib\"
                    \"libqquickvectorimageplugin.dylib\"
                    \"libqquickvectorimagehelpersplugin.dylib\"
                    \"libqmllocalstorageplugin.dylib\"
                    \"libqmlxmllistmodelplugin.dylib\"
                )
                foreach(pl IN LISTS unused_quick_plugins)
                    if(EXISTS \"\${plugins_dir}/quick/\${pl}\")
                        file(REMOVE \"\${plugins_dir}/quick/\${pl}\")
                        message(STATUS \"Pruned unused quick plugin: \${pl}\")
                    endif()
                endforeach()

                # 3. Remove QML modules for Controls styles this app never
                # uses (macdeployqt copies every installed style, incl. the
                # multi-MB FluentWinUI3/Imagine asset sets). Only Basic (the
                # fallback) and macOS (the native style) are kept.
                set(qml_dir \"\${CMAKE_INSTALL_PREFIX}/Immersion.app/Contents/Resources/qml\")
                set(unused_qml_dirs
                    \"QtQuick/Controls/FluentWinUI3\"
                    \"QtQuick/Controls/Material\"
                    \"QtQuick/Controls/Universal\"
                    \"QtQuick/Controls/Imagine\"
                    \"QtQuick/Controls/iOS\"
                    \"QtQuick/Controls/Fusion\"
                    \"QtQuick/Controls/designer\"
                    \"QtQuick/Particles\"
                    \"QtQuick/VectorImage\"
                )
                foreach(qd IN LISTS unused_qml_dirs)
                    if(EXISTS \"\${qml_dir}/\${qd}\")
                        file(REMOVE_RECURSE \"\${qml_dir}/\${qd}\")
                        message(STATUS \"Pruned unused QML module: \${qd}\")
                    endif()
                endforeach()

                # 4. Plugin dirs whose owning frameworks were pruned above
                # (e.g. sqldrivers depends on the removed QtSql).
                foreach(pd \"sqldrivers\")
                    if(EXISTS \"\${plugins_dir}/\${pd}\")
                        file(REMOVE_RECURSE \"\${plugins_dir}/\${pd}\")
                        message(STATUS \"Pruned unused plugin dir: \${pd}\")
                    endif()
                endforeach()
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
set(CPACK_PACKAGE_INSTALL_DIRECTORY "Immersion")

# Declare the main executable for installer shortcut generation.
set(CPACK_PACKAGE_EXECUTABLES "Immersion" "Immersion")
set(CPACK_CREATE_DESKTOP_LINKS "Immersion")

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
        set(CPACK_NSIS_DISPLAY_NAME "Immersion")
        set(CPACK_NSIS_PACKAGE_NAME "Immersion")
        set(CPACK_NSIS_EXECUTABLES_DIRECTORY ".")
        set(CPACK_NSIS_MENU_LINKS "Immersion.exe" "Immersion")
        set(CPACK_NSIS_CREATE_ICONS_EXTRA
            "CreateShortCut '$DESKTOP\\\\Immersion.lnk' '$INSTDIR\\\\Immersion.exe'")
        set(CPACK_NSIS_DELETE_ICONS_EXTRA
            "Delete '$DESKTOP\\\\Immersion.lnk'")
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

