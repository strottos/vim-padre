FROM ubuntu:18.04

RUN apt-get update \
    && apt-get -y upgrade \
    && apt-get install -y build-essential software-properties-common python-pip python3-pip git curl \
    && pip install six \
    && pip3 install six
RUN add-apt-repository -y ppa:jonathonf/vim \
    && apt-get update \
    && apt-get install -y vim

#ENV NODE_VERSION 10.16.0
#RUN mkdir /home/node \
#    && cd /home/node \
#    && curl -v -SLO "https://nodejs.org/dist/v$NODE_VERSION/node-v$NODE_VERSION.tar.xz" \
#    && curl -v -SLO --compressed "https://nodejs.org/dist/v$NODE_VERSION/SHASUMS256.txt.asc" \
#    && tar -xf "node-v$NODE_VERSION.tar.xz" \
#    && cd "node-v$NODE_VERSION" \
#    && ./configure \
#    && make -j$(getconf _NPROCESSORS_ONLN) \
#    && make install \
#    && cd / \
#    && rm -rf /home/node \

RUN apt-get install -y lldb-6.0 \
    && ln -s /usr/bin/lldb-6.0 /usr/bin/lldb

RUN useradd -m vim
USER vim

ENV PATH /home/vim/.cargo/bin:$PATH
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain nightly \
    && rustup toolchain install nightly \
    && rustup update

RUN mkdir -p /home/vim/.vim/autoload ~/.vim/bundle && curl -LSso /home/vim/.vim/autoload/pathogen.vim https://tpo.pe/pathogen.vim && git clone https://github.com/junegunn/vader.vim /home/vim/.vim/bundle/vader
COPY --chown=vim:vim test/ /home/vim/test/
COPY --chown=vim:vim test/vimrc /home/vim/.vimrc
COPY --chown=vim:vim padre /home/vim/.vim/bundle/vim-padre/padre
RUN cd /home/vim/.vim/bundle/vim-padre/padre && rm -rf target && cargo build
COPY --chown=vim:vim autoload /home/vim/.vim/bundle/vim-padre/autoload
COPY --chown=vim:vim plugin /home/vim/.vim/bundle/vim-padre/plugin
COPY --chown=vim:vim pythonx /home/vim/.vim/bundle/vim-padre/pythonx
RUN cd /home/vim/test/progs && rm -f test_prog && gcc -g -o test_prog test_prog.c test_func.c
WORKDIR /home/vim
