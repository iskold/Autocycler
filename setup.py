#!/usr/bin/env python3
"""
This is the Autocycler installation script. Assuming you're in the same directory, it can be run
like this: `python3 setup.py install`, or (probably better) like this: `pip3 install .`

Copyright 2023 Ryan Wick (rrwick@gmail.com)
https://github.com/rrwick/Autocycler

This file is part of Autocycler. Autocycler is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by the Free Software Foundation,
either version 3 of the License, or (at your option) any later version. Autocycler is distributed
in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
details. You should have received a copy of the GNU General Public License along with Autocycler.
If not, see <https://www.gnu.org/licenses/>.
"""

from setuptools import setup


def readme():
    with open('README.md') as f:
        return f.read()


# Get the program version from another file.
__version__ = '0.0.0'
exec(open('autocycler/version.py').read())


setup(name='Autocycler',
      version=__version__,
      description='Autocycler: a tool for producing consensus bacterial genome assemblies',
      long_description=readme(),
      long_description_content_type='text/markdown',
      url='https://github.com/rrwick/Autocycler',
      author='Ryan Wick',
      author_email='rrwick@gmail.com',
      license='GPLv3',
      packages=['autocycler'],
      install_requires=['pytest'],
      entry_points={"console_scripts": ['autocycler = autocycler.__main__:main']},
      include_package_data=True,
      zip_safe=False,
      python_requires='>=3.7')
