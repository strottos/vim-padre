FROM ubuntu:16.04

RUN apt-get update \
    && apt-get -y upgrade \
    && apt-get install -y software-properties-common python-software-properties build-essential libssl-dev curl git python-pip python3-pip \
    && pip install six \
    && pip3 install six
RUN add-apt-repository -y ppa:jonathonf/vim \
    && apt-get update \
    && apt-get install -y vim

ENV NVM_DIR /usr/local/nvm
ENV NODE_VERSION 8.11.2
RUN mkdir $NVM_DIR \
    && curl -o- https://raw.githubusercontent.com/creationix/nvm/v0.33.11/install.sh | bash \
    && . $NVM_DIR/nvm.sh \
    && nvm install $NODE_VERSION \
    && nvm alias default $NODE_VERSION \
    && nvm use default

RUN curl -o- https://apt.llvm.org/llvm-snapshot.gpg.key | apt-key add - \
    && apt-get install -y lldb-5.0 \
    && ln -s /usr/bin/lldb-5.0 /usr/bin/lldb \
    && ln -s /usr/bin/lldb-server-5.0 /usr/lib/llvm-5.0/bin/lldb-server-5.0.0

RUN useradd -m vim
USER vim

ENV NVM_DIR /usr/local/nvm
ENV NODE_VERSION 8.11.2
ENV NODE_PATH $NVM_DIR/versions/node/v$NODE_VERSION/lib/node_modules
ENV PATH      $NVM_DIR/versions/node/v$NODE_VERSION/bin:$PATH

RUN mkdir -p /home/vim/.vim/autoload ~/.vim/bundle && curl -LSso /home/vim/.vim/autoload/pathogen.vim https://tpo.pe/pathogen.vim && git clone https://github.com/junegunn/vader.vim /home/vim/.vim/bundle/vader
COPY --chown=vim:vim test/ /home/vim/test/
COPY --chown=vim:vim test/vimrc /home/vim/.vimrc
COPY --chown=vim:vim . /home/vim/.vim/bundle/vim-padre
RUN cd /home/vim/.vim/bundle/vim-padre/padre && rm -rf node_modules && npm install
RUN cd /home/vim/test/progs && rm test_prog && gcc -g -o test_prog test_prog.c test_func.c
WORKDIR /home/vim
