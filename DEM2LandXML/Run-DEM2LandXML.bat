@echo off
title DEM to LandXML Launcher
cd /d "%~dp0"

set "CONDA_ACTIVATE=%USERPROFILE%\anaconda3\Scripts\activate.bat"

if not exist "%CONDA_ACTIVATE%" (
    echo [ERROR] Could not find Anaconda activate script at:
    echo %CONDA_ACTIVATE%
    pause
    exit /b 1
)

call "%CONDA_ACTIVATE%" base

echo Checking required Python packages...
python -c "import rasterio, numpy, matplotlib, PIL, shapefile, pyproj" 2>nul
if errorlevel 1 (
    echo Installing missing packages...
    pip install rasterio numpy matplotlib pillow pyshp pyproj
    if errorlevel 1 (
        echo [ERROR] Package installation failed. Check your internet connection.
        pause
        exit /b 1
    )
)

echo Launching GeoTIFF DEM to LandXML Converter...
python "%~dp0gui.py"
exit /b 0
