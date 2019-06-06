FROM ubuntu:18.04

RUN apt-get update \
    && apt-get -y upgrade \
    && apt-get install -y software-properties-common python-pip python3-pip \
    && pip install six \
    && pip3 install six
RUN add-apt-repository -y ppa:jonathonf/vim \
    && apt-get update \
    && apt-get install -y vim

ENV NODE_VERSION 10.16.0
RUN apt-get install -y build-essential curl \
    && mkdir /home/node \
    && cd /home/node \
    && for key in \
           4ED778F539E3634C779C87C6D7062848A1AB005C \
           B9E2F5981AA6E0CD28160D9FF13993A75599653C \
           94AE36675C464D64BAFA68DD7434390BDBE9B9C5 \
           B9AE9905FFD7803F25714661B63B535A4C206CA9 \
           77984A986EBC2AA786BC0F66B01FBB92821C587A \
           71DCFD284A79C3B38668286BC97EC7A07EDE3FC1 \
           FD3A5288F042B6850C66B31F09FE44734EB7990E \
           8FCCA13FEF1D0C2E91008E09770F7A9A5AE15600 \
           C4F0DFFF4E8C1A8236409D08E73BC641CC11F4C8 \
           DD8F2338BAE7501E3DD5AC78C273792F7D83545D \
           A48C2BEE680E841632CD4E44F07496B3EB3C1762 \
       ; do \
           gpg --keyserver hkp://p80.pool.sks-keyservers.net:80 --recv-keys "$key" || \
           gpg --keyserver hkp://ipv4.pool.sks-keyservers.net --recv-keys "$key" || \
           gpg --keyserver hkp://pgp.mit.edu:80 --recv-keys "$key" ; \
       done \
    && curl -v -SLO "https://nodejs.org/dist/v$NODE_VERSION/node-v$NODE_VERSION.tar.xz" \
    && curl -v -SLO --compressed "https://nodejs.org/dist/v$NODE_VERSION/SHASUMS256.txt.asc" \
    && gpg --batch --decrypt --output SHASUMS256.txt SHASUMS256.txt.asc \
    && grep " node-v$NODE_VERSION.tar.xz\$" SHASUMS256.txt | sha256sum -c - \
    && tar -xf "node-v$NODE_VERSION.tar.xz" \
    && cd "node-v$NODE_VERSION" \
    && ./configure \
    && make -j$(getconf _NPROCESSORS_ONLN) \
    && make install \
    && cd / \
    && rm -rf /home/node

RUN curl -o- https://apt.llvm.org/llvm-snapshot.gpg.key | apt-key add - \
    && apt-get install -y lldb-5.0 \
    && ln -s /usr/bin/lldb-5.0 /usr/bin/lldb \
    && ln -s /usr/bin/lldb-server-5.0 /usr/lib/llvm-5.0/bin/lldb-server-5.0.0

RUN useradd -m vim
USER vim

ENV NODE_PATH $NVM_DIR/versions/node/v$NODE_VERSION/lib/node_modules
ENV PATH      $NVM_DIR/versions/node/v$NODE_VERSION/bin:$PATH

RUN mkdir -p /home/vim/.vim/autoload ~/.vim/bundle && curl -LSso /home/vim/.vim/autoload/pathogen.vim https://tpo.pe/pathogen.vim && git clone https://github.com/junegunn/vader.vim /home/vim/.vim/bundle/vader
COPY --chown=vim:vim test/ /home/vim/test/
COPY --chown=vim:vim test/vimrc /home/vim/.vimrc
COPY --chown=vim:vim . /home/vim/.vim/bundle/vim-padre
RUN cd /home/vim/.vim/bundle/vim-padre/padre && rm -rf node_modules && npm install
RUN cd /home/vim/test/progs && rm test_prog && gcc -g -o test_prog test_prog.c test_func.c
WORKDIR /home/vim
